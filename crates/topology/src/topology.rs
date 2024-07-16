use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::Address;
use alloy_provider::{ProviderBuilder, RootProvider};
use alloy_rpc_client::ClientBuilder;
use alloy_transport::BoxTransport;
use alloy_transport_ipc::IpcConnect;
use alloy_transport_ws::WsConnect;
use example_exex_remote::ExExClient;
use eyre::{eyre, OptionExt, Result};
use log::{error, info, warn};
use revm::db::{CacheDB, EmptyDB};
use tokio::task::JoinHandle;

use defi_actors::{BlockHistoryActor, EvmEstimatorActor, FlashbotsBroadcastActor, GasStationActor, GethEstimatorActor, HistoryPoolLoaderActor, InitializeSignersActor, MarketStatePreloadedActor, MempoolActor, NewPoolLoaderActor, NodeBlockActor, NodeExExGrpcActor, NodeMempoolActor, NonceAndBalanceMonitorActor, PoolHealthMonitorActor, PriceActor, ProtocolPoolLoaderActor, TxSignersActor};
use defi_blockchain::Blockchain;
use defi_entities::TxSigners;
use defi_types::ChainParameters;
use flashbots::Flashbots;
use loom_actors::{Accessor, Actor, Consumer, Producer, SharedState, WorkerResult};
use loom_multicaller::SwapStepEncoder;

use crate::topology_config::{BroadcasterConfig, ClientConfigParams, EncoderConfig, EstimatorConfig, SignersConfig, TopologyConfig};
use crate::topology_config::TransportType;

pub struct Topology
{
    clients: HashMap<String, ClientConfigParams>,
    blockchains: HashMap<String, Blockchain>,
    signers: HashMap<String, SharedState<TxSigners>>,
    encoders: HashMap<String, SwapStepEncoder>,
    default_blockchain_name: Option<String>,
    default_encoder_name: Option<String>,
    default_signer_name: Option<String>,

}


impl Topology
{
    pub async fn from(config: TopologyConfig) -> Result<(Topology, Vec<JoinHandle<WorkerResult>>)> {
        let mut topology = Topology {
            clients: HashMap::new(),
            blockchains: HashMap::new(),
            signers: HashMap::new(),
            encoders: HashMap::new(),
            default_blockchain_name: None,
            default_encoder_name: None,
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
                    let transport = WsConnect { url: config_params.url, auth: None };
                    ClientBuilder::default().ws(transport).await
                }
            };

            let client = if client.is_err() {
                error!("Error connecting to {name} error : {}", client.err().unwrap());
                continue;
            } else {
                client.unwrap()
            };

            let provider = Some(ProviderBuilder::new().on_client(client).boxed());

            topology.clients.insert(name.clone(), ClientConfigParams {
                provider,
                ..v.config_params()
            });
        }

        if topology.clients.is_empty() {
            return Err(eyre!("NO_CLIENTS_CONNECTED"));
        }


        for (k, v) in config.encoders.iter() {
            match v {
                EncoderConfig::SwapStep(c) => {
                    let address: Address = c.address.parse().unwrap();
                    let encoder = SwapStepEncoder::new(address);
                    topology.encoders.insert(k.clone(), encoder);
                    topology.default_encoder_name = Some(k.clone());
                }
            }
        }

        let chain_params = ChainParameters::ethereum();

        for (k, params) in config.blockchains.iter() {
            let blockchain = Blockchain::new(params.chain_id.unwrap_or(1));


            info!("Starting block history actor {k}");
            let mut block_history_actor = BlockHistoryActor::new();
            match block_history_actor
                .access(blockchain.latest_block())
                .access(blockchain.market_state())
                .access(blockchain.block_history())
                .consume(blockchain.new_block_headers_channel())
                .consume(blockchain.new_block_with_tx_channel())
                .consume(blockchain.new_block_logs_channel())
                .consume(blockchain.new_block_state_update_channel())
                .produce(blockchain.market_events_channel())
                .start().await {
                Ok(r) => {
                    tasks.extend(r);
                    info!("Block history actor started successfully")
                }
                Err(e) => {
                    panic!("{}", e)
                }
            }

            info!("Starting mempool actor {k}");
            let mut mempool_actor = MempoolActor::new(chain_params.clone());
            match mempool_actor
                .access(blockchain.mempool())
                .access(blockchain.block_history())
                .consume(blockchain.new_mempool_tx_channel())
                .consume(blockchain.market_events_channel())
                .produce(blockchain.mempool_events_channel())
                .start().await {
                Ok(r) => {
                    tasks.extend(r);
                    info!("Mempool actor started successfully")
                }
                Err(e) => {
                    panic!("{}", e)
                }
            }

            info!("Starting gas station actor {k}");
            let mut gas_station_actor = GasStationActor::new();
            match gas_station_actor
                .access(blockchain.gas_station())
                .access(blockchain.block_history())
                .consume(blockchain.market_events_channel())
                .produce(blockchain.market_events_channel())
                .start().await {
                Ok(r) => {
                    tasks.extend(r);
                    info!("Gas station actor started successfully")
                }
                Err(e) => {
                    panic!("{}", e)
                }
            }


            info!("Starting pool monitor monitor actor {k}");
            let mut new_pool_health_monior_actor = PoolHealthMonitorActor::new();
            match new_pool_health_monior_actor
                .access(blockchain.market())
                .consume(blockchain.pool_health_monitor_channel())
                .start().await {
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
                    let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();

                    let mut initialize_signers_actor = InitializeSignersActor::new_from_encrypted_env();
                    match initialize_signers_actor
                        .access(signers.clone())
                        .access(blockchain.nonce_and_balance())
                        .start().await {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Signers have been initialized")
                        }
                        Err(e) => {
                            panic!("Cannot initialize signers {e}");
                        }
                    }

                    let mut signers_actor = TxSignersActor::new();
                    match signers_actor
                        .consume(blockchain.compose_channel())
                        .produce(blockchain.compose_channel())
                        .start().await {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Signers actor has been started")
                        }
                        Err(e) => {
                            panic!("Cannot start signers actor {e}")
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

                let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                let client = topology.get_client(params.client.as_ref()).unwrap();
                let signers = topology.get_signers(params.signers.as_ref()).unwrap();

                let mut market_state_preload_actor = MarketStatePreloadedActor::new(client).with_signers(signers.clone()).with_encoder(&topology.get_encoder(None)?);
                match market_state_preload_actor
                    .access(blockchain.market_state())
                    .start().await {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Market state preload actor started successfully {name}")
                    }
                    Err(e) => {
                        panic!("MarketStatePreloadedActor : {e}")
                    }
                }
            }
        } else {
            warn!("No preloader in config")
        }


        if let Some(node_exex_actors) = config.actors.node_exex {
            for (name, params) in node_exex_actors {
                let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                let url = params.url.unwrap_or("http://[::1]:10000".to_string());


                info!("Starting node actor {name}");
                let mut node_exex_block_actor = NodeExExGrpcActor::new(url);
                match node_exex_block_actor
                    .produce(blockchain.new_block_headers_channel())
                    .produce(blockchain.new_block_with_tx_channel())
                    .produce(blockchain.new_block_logs_channel())
                    .produce(blockchain.new_block_state_update_channel())
                    .produce(blockchain.new_mempool_tx_channel())
                    .start().await {
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
                let client = topology.get_client(params.client.as_ref()).unwrap();
                let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                let client_config = topology.get_client_config(params.client.as_ref()).unwrap();

                info!("Starting node actor {name}");
                let mut node_block_actor = NodeBlockActor::new(client).with_reth_db(client_config.db_path);
                match node_block_actor
                    .produce(blockchain.new_block_headers_channel())
                    .produce(blockchain.new_block_with_tx_channel())
                    .produce(blockchain.new_block_logs_channel())
                    .produce(blockchain.new_block_state_update_channel())
                    .start().await {
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


        if let Some(node_mempool_actors) = config.actors.mempool {
            for (name, params) in node_mempool_actors {
                let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                match topology.get_client(params.client.as_ref()) {
                    Ok(client) => {
                        println!("Starting node mempool actor {name}");
                        let mut node_mempool_actor = NodeMempoolActor::new(client).with_name(name.clone());
                        match node_mempool_actor
                            .produce(blockchain.new_mempool_tx_channel())
                            .start().await {
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
                let client = topology.get_client(c.client.as_ref()).unwrap();
                let blockchain = topology.get_blockchain(c.blockchain.as_ref()).unwrap();
                info!("Starting price actor");
                let mut price_actor = PriceActor::new(client);
                match price_actor
                    .access(blockchain.market())
                    .start().await {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Price actor has been initialized : {name}")
                    }
                    Err(e) => {
                        panic!("Cannot initialize price actor {name} : {e}");
                    }
                }
            }
        } else {
            warn!("No price actor in config")
        }


        if let Some(node_balance_actors) = config.actors.noncebalance {
            for (name, c) in node_balance_actors {
                let client = topology.get_client(c.client.as_ref()).unwrap();
                let blockchain = topology.get_blockchain(c.blockchain.as_ref()).unwrap();

                info!("Starting nonce and balance monitor actor {name}");
                let mut nonce_and_balance_monitor = NonceAndBalanceMonitorActor::new(client);
                match nonce_and_balance_monitor
                    .access(blockchain.nonce_and_balance())
                    .access(blockchain.block_history())
                    .consume(blockchain.market_events_channel())
                    .start().await {
                    Ok(r) => {
                        tasks.extend(r);
                        info!("Nonce monitor has been initialized {name} for {}", blockchain.chain_id())
                    }
                    Err(e) => {
                        panic!("Cannot initialize nonce and balance monitor {name} : {e}");
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
                        let client = topology.get_client(params.client.as_ref()).unwrap();
                        let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();

                        let flashbots_client = Flashbots::new(client, "https://relay.flashbots.net");
                        let mut flashbots_actor = FlashbotsBroadcastActor::new(flashbots_client, params.smart.unwrap_or(false));
                        match flashbots_actor
                            .consume(blockchain.compose_channel())
                            .start().await {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("Flashbots broadcaster actor {name} started successfully for {}", blockchain.chain_id())
                            }
                            Err(e) => {
                                panic!("Error starting flashbots broadcaster actor {name} for {} : {e}", blockchain.chain_id())
                            }
                        }
                    }
                }
            }
        } else {
            warn!("No broadcaster actors in config")
        }


        if let Some(pool_actors) = config.actors.pools {
            for (name, params) in pool_actors {
                let client = topology.get_client(params.client.as_ref()).unwrap();
                let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                if params.history {
                    info!("Starting history pools loader {name}");

                    let mut history_pools_loader_actor = HistoryPoolLoaderActor::new(client.clone());
                    match history_pools_loader_actor
                        .access(blockchain.market())
                        .access(blockchain.market_state())
                        .start().await {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("History pool loader actor started successfully {name}")
                        }
                        Err(e) => {
                            panic!("HistoryPoolLoaderActor : {}", e)
                        }
                    }
                }
                if params.protocol {
                    info!("Starting protocols pools loader {name}");

                    let mut protocol_pools_loader_actor = ProtocolPoolLoaderActor::new(client.clone());
                    match protocol_pools_loader_actor
                        .access(blockchain.market())
                        .access(blockchain.market_state())
                        .start().await {
                        Err(e) => {
                            panic!("ProtocolPoolLoaderActor {e}")
                        }
                        Ok(r) => {
                            tasks.extend(r);
                            info!("Protocol pool loader actor started successfully")
                        }
                    }
                }

                if params.new {
                    info!("Starting new pool loader actor {name}");
                    let mut new_pool_actor = NewPoolLoaderActor::new(client.clone());
                    match new_pool_actor
                        .access(blockchain.market())
                        .access(blockchain.market_state())
                        .consume(blockchain.new_block_logs_channel())
                        .start().await {
                        Ok(r) => {
                            tasks.extend(r);
                            info!("New pool actor started")
                        }
                        Err(e) => { panic!("NewPoolLoaderActor : {}", e) }
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
                        let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                        let encoder = topology.get_encoder(params.encoder.as_ref()).unwrap();
                        let mut evm_estimator_actor = EvmEstimatorActor::new(encoder);
                        match evm_estimator_actor
                            .consume(blockchain.compose_channel())
                            .produce(blockchain.compose_channel())
                            .start().await {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("EVM estimator actor started successfully {name} @ {}", blockchain.chain_id())
                            }
                            Err(e) => {
                                panic!("Error starting EVM estimator actor {name} @ {} : {e}", blockchain.chain_id())
                            }
                        }
                    }
                    EstimatorConfig::Geth(params) => {
                        let client = topology.get_client(params.client.as_ref()).unwrap();
                        let blockchain = topology.get_blockchain(params.blockchain.as_ref()).unwrap();
                        let encoder = topology.get_encoder(params.encoder.as_ref()).unwrap();

                        let flashbots_client = Arc::new(Flashbots::new(client, "https://relay.flashbots.net"));

                        let mut geth_estimator_actor = GethEstimatorActor::new(flashbots_client, encoder);
                        match geth_estimator_actor
                            .consume(blockchain.compose_channel())
                            .produce(blockchain.compose_channel())
                            .start().await {
                            Ok(r) => {
                                tasks.extend(r);
                                info!("Geth estimator actor started successfully {name} @ {}", blockchain.chain_id())
                            }
                            Err(e) => {
                                panic!("Error starting Geth estimator actor for {name} @ {} : {e}", blockchain.chain_id())
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
            Some(a) => {
                Ok(a.client().ok_or_eyre("CLIENT_NOT_SET")?.clone())
            }
            None => { Err(eyre!("CLIENT_NOT_FOUND")) }
        }
    }

    pub fn get_client_config(&self, name: Option<&String>) -> Result<ClientConfigParams> {
        match self.clients.get(name.unwrap_or(&"local".to_string())) {
            Some(a) => {
                Ok(a.clone())
            }
            None => { Err(eyre!("CLIENT_NOT_FOUND")) }
        }
    }


    pub fn get_blockchain(&self, name: Option<&String>) -> Result<&Blockchain> {
        match self.blockchains.get(name.unwrap_or(&self.default_blockchain_name.clone().unwrap())) {
            Some(a) => { Ok(a) }
            None => { Err(eyre!("BLOCKCHAIN_NOT_FOUND")) }
        }
    }

    pub fn get_encoder(&self, name: Option<&String>) -> Result<SwapStepEncoder> {
        match self.encoders.get(name.unwrap_or(&self.default_encoder_name.clone().unwrap())) {
            Some(a) => { Ok(a.clone()) }
            None => { Err(eyre!("ENCODER_NOT_FOUND")) }
        }
    }

    pub fn get_signers(&self, name: Option<&String>) -> Result<SharedState<TxSigners>> {
        match self.signers.get(name.unwrap_or(&self.default_encoder_name.clone().unwrap())) {
            Some(a) => { Ok(a.clone()) }
            None => { Err(eyre!("SIGNERS_NOT_FOUND")) }
        }
    }
    pub fn get_mut_blockchain(&mut self, name: Option<&String>) -> Result<&mut Blockchain> {
        match self.blockchains.get_mut(name.unwrap_or(&self.default_blockchain_name.clone().unwrap())) {
            Some(a) => { Ok(a) }
            None => { Err(eyre!("CLIENT_NOT_FOUND")) }
        }
    }
}
