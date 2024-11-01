use std::collections::HashMap;
use std::sync::Arc;

use crate::topology_config::TransportType;
use crate::topology_config::{BroadcasterConfig, ClientConfigParams, EncoderConfig, EstimatorConfig, SignersConfig, TopologyConfig};
use alloy_primitives::Address;
use alloy_provider::{ProviderBuilder, RootProvider};
use alloy_rpc_client::ClientBuilder;
use alloy_transport::BoxTransport;
use alloy_transport_ipc::IpcConnect;
use alloy_transport_ws::WsConnect;
use eyre::{eyre, OptionExt, Result};
use loom_broadcast_accounts::{InitializeSignersOneShotBlockingActor, NonceAndBalanceMonitorActor, TxSignersActor};
use loom_broadcast_broadcaster::FlashbotsBroadcastActor;
use loom_broadcast_flashbots::Flashbots;
use loom_core_actors::{Accessor, Actor, Consumer, Producer, SharedState, WorkerResult};
use loom_core_block_history::BlockHistoryActor;
use loom_core_blockchain::Blockchain;
use loom_core_mempool::MempoolActor;
use loom_defi_health_monitor::PoolHealthMonitorActor;
use loom_defi_market::{CurvePoolLoaderOneShotActor, HistoryPoolLoaderOneShotActor, NewPoolLoaderActor, PoolLoaderActor};
use loom_defi_pools::PoolsConfig;
use loom_defi_preloader::MarketStatePreloadedOneShotActor;
use loom_defi_price::PriceActor;
use loom_execution_estimator::{EvmEstimatorActor, GethEstimatorActor};
use loom_execution_multicaller::MulticallerSwapEncoder;
use loom_node_actor_config::NodeBlockActorConfig;
#[cfg(feature = "db-access")]
use loom_node_db_access::RethDbAccessBlockActor;
use loom_node_grpc::NodeExExGrpcActor;
use loom_node_json_rpc::{NodeBlockActor, NodeMempoolActor};
use loom_types_entities::TxSigners;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

pub struct Topology {
    clients: HashMap<String, ClientConfigParams>,
    blockchains: HashMap<String, Blockchain>,
    signers: HashMap<String, SharedState<TxSigners>>,
    multicaller_encoders: HashMap<String, MulticallerSwapEncoder>,
    default_blockchain_name: Option<String>,
    default_multicaller_encoder_name: Option<String>,
    default_signer_name: Option<String>,
}

impl Topology {
    pub async fn from(config: TopologyConfig) -> Result<(Topology, Vec<JoinHandle<WorkerResult>>)> {
        let mut topology = Topology {
            clients: HashMap::new(),
            blockchains: HashMap::new(),
            signers: HashMap::new(),
            multicaller_encoders: HashMap::new(),
            default_blockchain_name: None,
            default_multicaller_encoder_name: None,
            default_signer_name: None,
        };

        let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

        //let timeout_duration = Duration::from_secs(10);

        for (name, v) in config.clients.clone().iter() {
            let config_params = v.config_params();

            info!("Connecting to {name} : {v:?}");

            let client = match config_params.transport {
                TransportType::Ipc => {
                    info!("Starting IPC connection");

                    let transport = IpcConnect::from(config_params.url);
                    ClientBuilder::default().ipc(transport).await
                }
                _ => {
                    info!("Starting WS connection");
                    let transport = WsConnect { url: config_params.url, auth: None, config: None };
                    ClientBuilder::default().ws(transport).await
                }
            };

            let client = match client {
                Ok(client) => client,
                Err(e) => {
                    error!("Error connecting to {name} error : {}", e);
                    continue;
                }
            };

            let provider = Some(ProviderBuilder::new().on_client(client).boxed());

            topology.clients.insert(name.clone(), ClientConfigParams { provider, ..v.config_params() });
        }

        if topology.clients.is_empty() {
            return Err(eyre!("NO_CLIENTS_CONNECTED"));
        }

        for (k, v) in config.encoders.iter() {
            match v {
                EncoderConfig::SwapStep(c) => {
                    let address: Address = c.address.parse()?;
                    let encoder = MulticallerSwapEncoder::new(address);
                    topology.multicaller_encoders.insert(k.clone(), encoder);
                    topology.default_multicaller_encoder_name = Some(k.clone());
                }
            }
        }

        for (k, params) in config.blockchains.iter() {
            let blockchain = Blockchain::new(params.chain_id.unwrap_or(1) as u64);

            info!("Starting block history actor {k}");
            let mut block_history_actor = BlockHistoryActor::new(topology.get_client(None)?);
            match block_history_actor
                .access(blockchain.latest_block())
                .access(blockchain.market_state())
                .access(blockchain.block_history())
                .consume(blockchain.new_block_headers_channel())
                .consume(blockchain.new_block_with_tx_channel())
                .consume(blockchain.new_block_logs_channel())
                .consume(blockchain.new_block_state_update_channel())
                .produce(blockchain.market_events_channel())
                .start()
            {
                Ok(r) => {
                    tasks.extend(r);
                    info!("Block history actor started successfully")
                }
                Err(e) => {
                    panic!("{}", e)
                }
            }

            info!("Starting mempool actor {k}");
            let mut mempool_actor = MempoolActor::new();
            match mempool_actor
                .access(blockchain.mempool())
                .consume(blockchain.new_mempool_tx_channel())
                .consume(blockchain.new_block_headers_channel())
                .consume(blockchain.new_block_with_tx_channel())
                .produce(blockchain.mempool_events_channel())
                .start()
            {
                Ok(r) => {
                    tasks.extend(r);
                    info!("Mempool actor started successfully")
                }
                Err(e) => {
                    panic!("{}", e)
                }
            }

            info!("Starting pool monitor monitor actor {k}");
            let mut new_pool_health_monior_actor = PoolHealthMonitorActor::new();
            match new_pool_health_monior_actor.access(blockchain.market()).consume(blockchain.pool_health_monitor_channel()).start() {
                Ok(r) => {
                    tasks.extend(r);
                    info!("Pool monitor monitor actor started")
                }
                Err(e) => {
                    panic!("PoolHealthMonitorActor error {}", e)
                }
            }

            topology.blockchains.insert(k.clone(), blockchain);
            topology.default_blockchain_name = Some(k.clone());
        }

        for (name, params) in config.signers.iter() {
            let signers = SharedState::new(TxSigners::new());
            match params {
                SignersConfig::Env(params) => {
                    info!("Starting initialize env signers actor {name}");
                    let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;

                    let mut initialize_signers_actor = InitializeSignersOneShotBlockingActor::new_from_encrypted_env();
                    match initialize_signers_actor.access(signers.clone()).access(blockchain.nonce_and_balance()).start_and_wait() {
                        Ok(_) => {
                            info!("Signers have been initialized")
                        }
                        Err(e) => {
                            panic!("Cannot initialize signers {}", e);
                        }
                    }

                    let mut signers_actor = TxSignersActor::new();
                    match signers_actor.consume(blockchain.compose_channel()).produce(blockchain.compose_channel()).start() {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Signers actor has been started")
                        }
                        Err(e) => {
                            panic!("Cannot start signers actor {}", e)
                        }
                    }
                    topology.signers.insert(name.clone(), signers);
                    topology.default_signer_name = Some(name.clone());
                }
            }
        }

        if let Some(preloader_actors) = config.preloaders {
            for (name, params) in preloader_actors {
                info!("Starting market state preload actor {name}");

                let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                let client = topology.get_client(params.client.as_ref())?;
                let signers = topology.get_signers(params.signers.as_ref())?;

                let mut market_state_preload_actor = MarketStatePreloadedOneShotActor::new(client)
                    .with_signers(signers.clone())
                    .with_copied_account(topology.get_multicaller_encoder(None)?.get_contract_address());
                match market_state_preload_actor.access(blockchain.market_state()).start_and_wait() {
                    Ok(_) => {
                        info!("Market state preload actor executed successfully")
                    }
                    Err(e) => {
                        panic!("MarketStatePreloadedOneShotActor : {}", e)
                    }
                }
            }
        } else {
            warn!("No preloader in config")
        }

        if let Some(node_exex_actors) = config.actors.node_exex {
            for (name, params) in node_exex_actors {
                let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                let url = params.url.unwrap_or("http://[::1]:10000".to_string());

                info!("Starting node actor {name}");
                let mut node_exex_block_actor = NodeExExGrpcActor::new(url);
                match node_exex_block_actor
                    .produce(blockchain.new_block_headers_channel())
                    .produce(blockchain.new_block_with_tx_channel())
                    .produce(blockchain.new_block_logs_channel())
                    .produce(blockchain.new_block_state_update_channel())
                    .produce(blockchain.new_mempool_tx_channel())
                    .start()
                {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Node ExEx actor started successfully for : {} @ {}", name, blockchain.chain_id())
                    }
                    Err(e) => {
                        panic!("{}", e)
                    }
                }
            }
        }

        if let Some(node_block_actors) = config.actors.node {
            for (name, params) in node_block_actors {
                let client = topology.get_client(params.client.as_ref())?;
                let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                let client_config = topology.get_client_config(params.client.as_ref())?;

                info!("Starting node actor {name}");

                #[cfg(feature = "db-access")]
                if client_config.db_path.is_some() {
                    let mut node_block_actor = RethDbAccessBlockActor::new(
                        client.clone(),
                        NodeBlockActorConfig::all_enabled(),
                        client_config.db_path.clone().unwrap_or_default(),
                    );
                    match node_block_actor
                        .produce(blockchain.new_block_headers_channel())
                        .produce(blockchain.new_block_with_tx_channel())
                        .produce(blockchain.new_block_logs_channel())
                        .produce(blockchain.new_block_state_update_channel())
                        .start()
                    {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Reth db access node actor started successfully for : {} @ {}", name, blockchain.chain_id())
                        }
                        Err(e) => {
                            panic!("{}", e)
                        }
                    }
                }

                if client_config.db_path.is_none() {
                    let mut node_block_actor = NodeBlockActor::new(client, NodeBlockActorConfig::all_enabled());
                    match node_block_actor
                        .produce(blockchain.new_block_headers_channel())
                        .produce(blockchain.new_block_with_tx_channel())
                        .produce(blockchain.new_block_logs_channel())
                        .produce(blockchain.new_block_state_update_channel())
                        .start()
                    {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Node actor started successfully for : {} @ {}", name, blockchain.chain_id())
                        }
                        Err(e) => {
                            panic!("{}", e)
                        }
                    }
                }
            }
        }

        if let Some(node_mempool_actors) = config.actors.mempool {
            for (name, params) in node_mempool_actors {
                let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                match topology.get_client(params.client.as_ref()) {
                    Ok(client) => {
                        println!("Starting node mempool actor {name}");
                        let mut node_mempool_actor = NodeMempoolActor::new(client).with_name(name.clone());
                        match node_mempool_actor.produce(blockchain.new_mempool_tx_channel()).start() {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("Node mempool actor started successfully {name}")
                            }
                            Err(e) => {
                                panic!("{}", e)
                            }
                        }
                    }
                    Err(e) => {
                        error!("Skipping mempool actor for {} @ {} : {}", name, blockchain.chain_id(), e)
                    }
                }
            }
        }

        if let Some(price_actors) = config.actors.price {
            for (name, c) in price_actors {
                let client = topology.get_client(c.client.as_ref())?;
                let blockchain = topology.get_blockchain(c.blockchain.as_ref())?;
                info!("Starting price actor");
                let mut price_actor = PriceActor::new(client);
                match price_actor.access(blockchain.market()).start() {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Price actor has been initialized : {}", name)
                    }
                    Err(e) => {
                        panic!("Cannot initialize price actor {} : {}", name, e);
                    }
                }
            }
        } else {
            warn!("No price actor in config")
        }

        if let Some(node_balance_actors) = config.actors.noncebalance {
            for (name, c) in node_balance_actors {
                let client = topology.get_client(c.client.as_ref())?;
                let blockchain = topology.get_blockchain(c.blockchain.as_ref())?;

                info!("Starting nonce and balance monitor actor {name}");
                let mut nonce_and_balance_monitor = NonceAndBalanceMonitorActor::new(client);
                match nonce_and_balance_monitor
                    .access(blockchain.nonce_and_balance())
                    .access(blockchain.block_history())
                    .consume(blockchain.market_events_channel())
                    .start()
                {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Nonce monitor has been initialized {name} for {}", blockchain.chain_id())
                    }
                    Err(e) => {
                        panic!("Cannot initialize nonce and balance monitor {} : {}", name, e);
                    }
                }
            }
        } else {
            warn!("No nonce and balance actors in config");
        }

        if let Some(broadcaster_actors) = config.actors.broadcaster {
            for (name, params) in broadcaster_actors {
                match params {
                    BroadcasterConfig::Flashbots(params) => {
                        let client = topology.get_client(params.client.as_ref())?;
                        let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;

                        let flashbots_client = Flashbots::new(client, "https://relay.flashbots.net", None).with_default_relays();
                        let mut flashbots_actor = FlashbotsBroadcastActor::new(flashbots_client, params.smart.unwrap_or(false), true);
                        match flashbots_actor.consume(blockchain.compose_channel()).start() {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("Flashbots broadcaster actor {name} started successfully for {}", blockchain.chain_id())
                            }
                            Err(e) => {
                                panic!("Error starting flashbots broadcaster actor {name} for {} : {}", blockchain.chain_id(), e)
                            }
                        }
                    }
                }
            }
        } else {
            warn!("No broadcaster actors in config")
        }

        if let Some(pool_actors) = config.actors.pools {
            let mut blockchains = HashMap::new();
            for (name, params) in pool_actors {
                let client = topology.get_client(params.client.as_ref())?;
                let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                blockchains.insert(blockchain.chain_id(), blockchain);
                if params.history {
                    info!("Starting history pools loader {name}");

                    let mut history_pools_loader_actor = HistoryPoolLoaderOneShotActor::new(client.clone(), PoolsConfig::new());
                    match history_pools_loader_actor.produce(blockchain.tasks_channel()).start() {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("History pool loader actor started successfully {name}")
                        }
                        Err(e) => {
                            panic!("HistoryPoolLoaderOneShotActor : {}", e)
                        }
                    }
                }
                if params.protocol {
                    info!("Starting curve pools loader {name}");

                    let mut curve_pools_loader_actor = CurvePoolLoaderOneShotActor::new(client.clone());
                    match curve_pools_loader_actor.access(blockchain.market()).access(blockchain.market_state()).start() {
                        Err(e) => {
                            panic!("CurvePoolLoaderOneShotActor : {}", e)
                        }
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Curve pool loader actor started successfully")
                        }
                    }
                }

                if params.new {
                    info!("Starting new pool loader actor {name}");
                    let mut new_pool_actor = NewPoolLoaderActor::new(PoolsConfig::new());
                    match new_pool_actor.consume(blockchain.new_block_logs_channel()).produce(blockchain.tasks_channel()).start() {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("New pool actor started")
                        }
                        Err(e) => {
                            panic!("NewPoolLoaderActor : {}", e)
                        }
                    }
                }

                info!("Starting pool loader actor {name}");
                let mut pool_loader_actor = PoolLoaderActor::new(client.clone());
                match pool_loader_actor
                    .access(blockchain.market())
                    .access(blockchain.market_state())
                    .consume(blockchain.tasks_channel())
                    .start()
                {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Pool loader actor started successfully")
                    }
                    Err(e) => {
                        panic!("PoolLoaderActor : {}", e)
                    }
                }
            }
        } else {
            warn!("No pool loader actors in config")
        }

        if let Some(estimator_actors) = config.actors.estimator {
            for (name, params) in estimator_actors {
                match params {
                    EstimatorConfig::Evm(params) => {
                        let client = params.client.as_ref().map(|x| topology.get_client(Some(x))).transpose()?; //   topology.get_client(params.client.as_ref())?;

                        let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                        let encoder = topology.get_multicaller_encoder(params.encoder.as_ref())?;
                        let mut evm_estimator_actor = EvmEstimatorActor::new_with_provider(encoder, client);
                        match evm_estimator_actor.consume(blockchain.compose_channel()).produce(blockchain.compose_channel()).start() {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("EVM estimator actor started successfully {name} @ {}", blockchain.chain_id())
                            }
                            Err(e) => {
                                panic!("Error starting EVM estimator actor {name} @ {} : {}", blockchain.chain_id(), e)
                            }
                        }
                    }
                    EstimatorConfig::Geth(params) => {
                        let client = topology.get_client(params.client.as_ref())?;
                        let blockchain = topology.get_blockchain(params.blockchain.as_ref())?;
                        let encoder = topology.get_multicaller_encoder(params.encoder.as_ref())?;

                        let flashbots_client = Arc::new(Flashbots::new(client, "https://relay.flashbots.net", None).with_default_relays());

                        let mut geth_estimator_actor = GethEstimatorActor::new(flashbots_client, encoder);
                        match geth_estimator_actor.consume(blockchain.compose_channel()).produce(blockchain.compose_channel()).start() {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("Geth estimator actor started successfully {name} @ {}", blockchain.chain_id())
                            }
                            Err(e) => {
                                panic!("Error starting Geth estimator actor for {name} @ {} : {}", blockchain.chain_id(), e)
                            }
                        }
                    }
                }
            }
        } else {
            warn!("No estimator actors in config")
        }

        Ok((topology, tasks))
    }

    pub fn get_client(&self, name: Option<&String>) -> Result<RootProvider<BoxTransport>> {
        match self.clients.get(name.unwrap_or(&"local".to_string())) {
            Some(a) => Ok(a.client().ok_or_eyre("CLIENT_NOT_SET")?.clone()),
            None => Err(eyre!("CLIENT_NOT_FOUND")),
        }
    }

    pub fn get_client_config(&self, name: Option<&String>) -> Result<ClientConfigParams> {
        match self.clients.get(name.unwrap_or(&"local".to_string())) {
            Some(a) => Ok(a.clone()),
            None => Err(eyre!("CLIENT_NOT_FOUND")),
        }
    }

    pub fn get_blockchain(&self, name: Option<&String>) -> Result<&Blockchain> {
        match self.blockchains.get(name.unwrap_or(&self.default_blockchain_name.clone().unwrap())) {
            Some(a) => Ok(a),
            None => Err(eyre!("BLOCKCHAIN_NOT_FOUND")),
        }
    }

    pub fn get_multicaller_encoder(&self, name: Option<&String>) -> Result<MulticallerSwapEncoder> {
        match self.multicaller_encoders.get(name.unwrap_or(&self.default_multicaller_encoder_name.clone().unwrap())) {
            Some(encoder) => Ok(encoder.clone()),
            None => Err(eyre!("ENCODER_NOT_FOUND")),
        }
    }

    pub fn get_signers(&self, name: Option<&String>) -> Result<SharedState<TxSigners>> {
        match self.signers.get(name.unwrap_or(&self.default_multicaller_encoder_name.clone().unwrap())) {
            Some(a) => Ok(a.clone()),
            None => Err(eyre!("SIGNERS_NOT_FOUND")),
        }
    }
    pub fn get_mut_blockchain(&mut self, name: Option<&String>) -> Result<&mut Blockchain> {
        match self.blockchains.get_mut(name.unwrap_or(&self.default_blockchain_name.clone().unwrap())) {
            Some(a) => Ok(a),
            None => Err(eyre!("CLIENT_NOT_FOUND")),
        }
    }
}
