use std::collections::BTreeMap;
use std::convert::Infallible;
use std::panic::panic_any;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy_node_bindings::Anvil;
use alloy_primitives::{Address, BlockHash, BlockNumber, TxHash, U256};
use alloy_provider::{Provider, ProviderBuilder, ProviderLayer};
use alloy_provider::layers::AnvilLayer;
use alloy_rpc_types::{Block, BlockId, BlockNumberOrTag, Header, Log};
use alloy_transport_http::reqwest::Url;
use alloy_transport_ws::WsConnect;
use clap::Parser;
use eyre::Result;
use log::{debug, error, info, LevelFilter};
use revm::db::EmptyDB;
use revm::InMemoryDB;

use debug_provider::{AnvilDebugProvider, DebugProviderExt};
use defi_actors::{
    ArbSwapPathEncoderActor, BlockHistoryActor, DiffPathMergerActor,
    EvmEstimatorActor, fetch_and_add_pool_by_address, fetch_state_and_add_pool, GasStationActor,
    HardhatBroadcastActor, InitializeSignersActor, MarketStatePreloadedActor, NodeBlockActor,
    NonceAndBalanceMonitorActor, PriceActor, SamePathMergerActor, StateChangeArbActor, StateChangeArbSearcherActor,
};
use defi_entities::{
    AccountNonceAndBalanceState, BlockHistory, GasStation, LatestBlock, Market, MarketState, Pool,
    PoolClass, PoolWrapper, Token, TxSigners,
};
use defi_events::{
    BlockLogsUpdate, BlockStateUpdate, MarketEvents, MempoolEvents, MessageHealthEvent,
    MessageMempoolDataUpdate, MessageTxCompose,
};
use defi_pools::CurvePool;
use defi_pools::protocols::CurveProtocol;
use defi_types::{ChainParameters, debug_trace_block, GethStateUpdateVec, Mempool};
use loom_actors::{Accessor, Actor, Broadcaster, Consumer, Producer, SharedState};
use loom_multicaller::SwapStepEncoder;

use crate::test_config::TestConfig;

mod test_config;
mod default;


fn parse_tx_hashes(tx_hash_vec: Vec<&str>) -> Result<Vec<TxHash>> {
    let mut ret: Vec<TxHash> = Vec::new();
    for tx_hash in tx_hash_vec {
        ret.push(tx_hash.parse()?);
    }
    Ok(ret)
}

#[derive(Parser, Debug)]
struct Commands {
    #[arg(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .format_timestamp_micros()
        .filter_level(LevelFilter::Debug)
        .init();


    let multicaller_address: Address = "0x0000000000000000000000000000000000000000"
        .parse()
        .unwrap(); // ETH values
    let encoder = Arc::new(SwapStepEncoder::new(multicaller_address));


    let args = Commands::parse();
    let test_config = TestConfig::from_file(args.config).await?;
    info!("{:?}", test_config);


    let node_url = Url::parse("ws://falcon.loop:8008/looper")?;
    //let anvil_ws = WsConnect::new(Url::parse("ws://127.0.0.1:8545")?);
    let node_ws = WsConnect::new(node_url.clone());

    let anvil = Anvil::new().fork_block_number(test_config.settings.block).fork(node_url.clone()).chain_id(1);

    let priv_key = anvil.clone().spawn().keys()[0].clone();


    let anvil_layer = AnvilLayer::from(anvil.clone());
    let anvil_url = anvil_layer.ws_endpoint_url();
    let anvil_ws = WsConnect::new(anvil_url.clone());


    let anvil_provider = ProviderBuilder::new().on_ws(anvil_ws).await?;

    //let anvil_provider = ProviderBuilder::new().on_http(anvil_provider.root()).await?.boxed();

    let node_provider = ProviderBuilder::new().on_ws(node_ws).await?.boxed();
    let provider = AnvilDebugProvider::new(node_provider, anvil_provider, BlockNumberOrTag::Latest);
    let client = provider;

    let block_nr = client.get_block_number().await?;
    info!("Block : {}", block_nr);

    let block_header = client
        .get_block(block_nr.into(), false)
        .await
        .unwrap()
        .unwrap()
        .header;

    let block_header_with_txes = client
        .get_block(block_nr.into(), true)
        .await
        .unwrap()
        .unwrap();

    let mut cache_db = InMemoryDB::new(EmptyDB::new());

    let mut market_instance = Market::default();

    let mut market_state_instance = MarketState::new(cache_db.clone());

    let mut mempool_instance = Mempool::new();

    info!("Creating channels");
    let new_block_headers_channel: Broadcaster<Header> = Broadcaster::new(10);
    let new_block_with_tx_channel: Broadcaster<Block> = Broadcaster::new(10);
    let new_block_state_update_channel: Broadcaster<BlockStateUpdate> = Broadcaster::new(10);
    let new_block_logs_channel: Broadcaster<BlockLogsUpdate> = Broadcaster::new(10);

    let new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate> = Broadcaster::new(500);

    let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(100);
    let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(500);
    let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(100);


    let mut market_instance = SharedState::new(market_instance);

    let mut market_state = SharedState::new(market_state_instance);

    let mut mempool_instance = SharedState::new(mempool_instance);
    let mut gas_station_state = SharedState::new(GasStation::new());

    let mut block_history_state =
        SharedState::new(BlockHistory::fetch(client.clone(), market_state.inner(), 10).await?);

    let mut tx_signers = TxSigners::new();
    let mut accounts_state = AccountNonceAndBalanceState::new();

    let mut tx_signers = SharedState::new(tx_signers);
    let mut accounts_state = SharedState::new(accounts_state);

    let block_hash: BlockHash = block_header.hash.unwrap_or_default();

    let mut latest_block = SharedState::new(LatestBlock::new(block_nr, block_hash));

    let (_, post) = debug_trace_block(
        client.clone(),
        BlockId::Number(BlockNumberOrTag::Number(block_nr)),
        true,
    )
        .await?;
    latest_block.write().await.update(
        block_nr,
        block_hash,
        Some(block_header.clone()),
        Some(block_header_with_txes),
        None,
        Some(post),
    );


    for (pool_name, pool_config) in test_config.pools {
        match pool_config.class {
            PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
                fetch_and_add_pool_by_address(
                    client.clone(),
                    market_instance.clone(),
                    market_state.clone(),
                    pool_config.address,
                    pool_config.class,
                )
                    .await?;
            }
            PoolClass::Curve => {
                if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(
                    client.clone(),
                    pool_config.address,
                )
                    .await
                {
                    let curve_pool =
                        CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
                    fetch_state_and_add_pool(
                        client.clone(),
                        market_instance.clone(),
                        market_state.clone(),
                        curve_pool.into(),
                    )
                        .await?
                } else {
                    error!("CURVE_POOL_NOT_LOADED");
                }
            }
            _ => {
                error!("Unknown pool class")
            }
        }
    }

    for (token_name, token_config) in test_config.tokens {
        let symbol = token_config
            .symbol
            .unwrap_or(token_config.address.to_checksum(None));
        let name = token_config.name.unwrap_or(symbol.clone());
        let token = Token::new_with_data(
            token_config.address,
            Some(symbol),
            Some(name),
            Some(token_config.decimals.map_or(18, |x| x)),
            token_config.basic.unwrap_or_default(),
            token_config.middle.unwrap_or_default(),
        );
        market_instance.write().await.add_token(token);
    }

    let chain_params = ChainParameters::ethereum();

    info!("Starting initialize signers actor");

    let mut initialize_signers_actor = InitializeSignersActor::new(Some(priv_key.to_bytes().to_vec()));
    match initialize_signers_actor
        .access(tx_signers.clone())
        .access(accounts_state.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e);
            panic!("Cannot initialize signers");
        }
        _ => info!("Signers have been initialized"),
    }

    info!("Starting market state preload actor");
    let mut market_state_preload_actor =
        MarketStatePreloadedActor::new(client.clone(), encoder.clone());
    match market_state_preload_actor
        .access(market_state.clone())
        .access(tx_signers.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Market state preload actor started successfully")
        }
    }

    //load_pools(client.clone(), market_instance.clone(), market_state.clone()).await?;

    info!("Starting node actor");
    let mut node_block_actor = NodeBlockActor::new(client.clone());
    match node_block_actor
        .produce(new_block_headers_channel.clone())
        .produce(new_block_with_tx_channel.clone())
        .produce(new_block_logs_channel.clone())
        .produce(new_block_state_update_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Node actor started successfully")
        }
    }

    info!("Starting nonce and balance monitor actor");
    let mut nonce_and_balance_monitor = NonceAndBalanceMonitorActor::new(client.clone());
    match nonce_and_balance_monitor
        .access(accounts_state.clone())
        .access(block_history_state.clone())
        .consume(market_events_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e);
            panic!("Cannot initialize nonce and balance monitor");
        }
        _ => info!("Nonce monitor has been initialized"),
    }

    info!("Starting price actor");
    let mut price_actor = PriceActor::new(client.clone());
    match price_actor.access(market_instance.clone()).start().await {
        Err(e) => {
            error!("{}", e);
            panic!("Cannot initialize price actor");
        }
        _ => info!("Price actor has been initialized"),
    }

    info!("Starting gas station actor");
    let mut gas_station_actor = GasStationActor::new(chain_params.clone());
    match gas_station_actor
        .access(gas_station_state.clone())
        .access(block_history_state.clone())
        .consume(market_events_channel.clone())
        .produce(market_events_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Gas station actor started successfully")
        }
    }

    info!("Starting block history actor");
    let mut block_history_actor = BlockHistoryActor::new(chain_params.clone());
    match block_history_actor
        .access(latest_block.clone())
        .access(market_state.clone())
        .access(block_history_state.clone())
        .consume(new_block_headers_channel.clone())
        .consume(new_block_with_tx_channel.clone())
        .consume(new_block_logs_channel.clone())
        .consume(new_block_state_update_channel.clone())
        .produce(market_events_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Block history actor started successfully")
        }
    }

    let tx_compose_channel: Broadcaster<MessageTxCompose> = Broadcaster::new(100);

    let mut broadcast_actor = HardhatBroadcastActor::new(client.clone());
    match broadcast_actor
        .access(latest_block.clone())
        .consume(tx_compose_channel.clone())
        .start()
        .await
    {
        Err(e) => error!("{}", e),
        _ => {
            info!("Broadcast actor started successfully")
        }
    }

    //let mut estimator_actor = HardhatEstimatorActor::new(client.clone(), encoder.clone());
    let mut estimator_actor = EvmEstimatorActor::new(encoder.clone());
    match estimator_actor
        .consume(tx_compose_channel.clone())
        .produce(tx_compose_channel.clone())
        .start()
        .await
    {
        Err(e) => error!("{e}"),
        _ => {
            info!("Estimate actor started successfully")
        }
    }

    /*
    info!("Starting signers actor");
    let mut signers_actor = SignersActor::new();
    match signers_actor
        .access(tx_signers.clone())
        .consume(tx_compose_channel.clone())
        .produce(tx_compose_channel.clone())
        .start().await {
        Err(e)=>{
            error!("{}",e);
            panic!("Cannot start signers");
        }
        _=>info!("Signers actor started")
    }
     */

    /*
    info!("Starting state change arb actor");
    let mut state_change_arb_actor = StateChangeArbSearcherActor::new(true);
    match state_change_arb_actor
        .access(market_instance.clone())
        .consume()
        .produce(tx_compose_channel.clone())
        .produce(pool_health_monitor_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("State change arb actor started successfully")
        }
    }

     */

    info!("Starting state change arb actor");
    let mut state_change_arb_actor = StateChangeArbActor::new(client.clone(), false, true);
    match state_change_arb_actor
        .access(mempool_instance.clone())
        .access(latest_block.clone())
        .access(market_instance.clone())
        .access(market_state.clone())
        .access(block_history_state.clone())
        .consume(market_events_channel.clone())
        .consume(mempool_events_channel.clone())
        .produce(tx_compose_channel.clone())
        .produce(pool_health_monitor_channel.clone())
        .start().await {
        Err(e) => { error!("{}",e) }
        _ => { info!("State change arb actor started successfully") }
    }

    info!("Starting swap path encoder actor");

    let mut swap_path_encoder_actor = ArbSwapPathEncoderActor::new(multicaller_address);

    match swap_path_encoder_actor
        .access(mempool_instance.clone())
        //.access(market_state.clone())
        .access(tx_signers.clone())
        .access(accounts_state.clone())
        .access(latest_block.clone())
        .consume(tx_compose_channel.clone())
        .produce(tx_compose_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Swap path encoder actor started successfully")
        }
    }

    info!("Starting swap path merger actor");

    /*
    Merger
    let mut swap_path_merger_actor = ArbSwapPathMergerActor::new(
        multicaller_address
    );
    match swap_path_merger_actor
        .access(mempool_instance.clone())
        .access(market_state.clone())
        .access(tx_signers.clone())
        .access( accounts_state.clone())
        .access( latest_block.clone())
        .consume( tx_compose_channel.clone())
        .consume( market_events_channel.clone())
        .produce( tx_compose_channel.clone())
        .start().await {
        Err(e)=>{error!("{}",e)}
        _=>{info!("Swap path merger actor started successfully")}
    }

     */

    let mut same_path_merger_actor = SamePathMergerActor::new(client.clone());
    match same_path_merger_actor
        .access(mempool_instance.clone())
        .access(market_state.clone())
        .access(tx_signers.clone())
        .access(accounts_state.clone())
        .access(latest_block.clone())
        .consume(tx_compose_channel.clone())
        .consume(market_events_channel.clone())
        .produce(tx_compose_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Same path merger actor started successfully")
        }
    }

    let mut diff_path_merger_actor = DiffPathMergerActor::new();
    match diff_path_merger_actor
        .access(mempool_instance.clone())
        .access(market_state.clone())
        .access(tx_signers.clone())
        .access(accounts_state.clone())
        .access(latest_block.clone())
        .consume(tx_compose_channel.clone())
        .consume(market_events_channel.clone())
        .produce(tx_compose_channel.clone())
        .start()
        .await
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Same path merger actor started successfully")
        }
    }

    let market_events_channel_clone = market_events_channel.clone();
    let client_clone = client.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        info!("rebroadcaster");
        //let tx_hash : H256 = "0x912c5773a432559796a3b35e49ceaef2420884e2a67165fadb81c0e4538c1ce2".parse().unwrap();
        //let tx_hash : H256 = "0xefdd9c15c418d689e43d0356131339ed214a0122264573dfe8421733368e878f".parse().unwrap();
        //let txs_hash : Vec<H256> = vec!["0x1114432ef38437dde663aeb868f553e0ec0ca973120e472687957223efeda331".parse().unwrap()]; //18498188
        //let tx_hash : H256 = "0x26177953373b45fa2abac4dee9634c7db65a9e0aaf64b99c7095f51d229f24b7".parse().unwrap(); //18498188
        //let tx_hash : H256 = "0x26177953373b45fa2abac4dee9634c7db65a9e0aaf64b99c7095f51d229f24b7".parse().unwrap(); //18498188
        /*
        let txs_hash : Vec<H256> = vec![
            "0x037c66ae5e0e893c4f47ef47d21f0afc18fdad334f92e898cae1f2a3da92f9b3".parse().unwrap(),
            "0x054a3f0c4ff3cf582c167669ed845f50b39f92007683c03b2ea53c522749d215".parse().unwrap(),
        ]; //18567699
         */
        //let txs_send_19101579 = vec!["0xce5ff199495cb0e47cb4e35749ba8263fbe8428ef5b895676dfb07c784d127d8"];

        let txs_19101579 = vec![
            "0x57593a2ca17101536d5b0a98afa17d5bb24eff8370b4d43859f45c27043184a1",
            "0xa77549d2a9fe1e7fcf54619af4f79fd36cdb76f287dfd1926f5d4dca92d7147e",
            "0xc8fa479a462b43545fe7dd05b375b6ff57c9d961c76e8955e44b9f604b7e60a4",
            "0x46081e7e9feed67e378cf743fb56355ce510441f6dad16f69f47e5dbb13ddd50",
            "0x0def9bd26edcd94ad3d9a7269de062d2bf34682f25c2fdcae91360241fd82351",
            "0x505ef4f817f97da840ca09a811d2d6a185bbb889f5afb9817ad74dc86b5419f7",
        ];

        let txs_19109955 = vec![
            "0xf9fb98fe76dc5f4e836cdc3d80cd7902150a8609c617064f1447c3980fd6776b",
            "0x1ec982c2d4eb5475192b26f7208b797328eab88f8e5be053f797f74bcb87a20c",
        ];

        //let snap_id = client.dev_rpc().snapshot().await.unwrap();

        let txs_hash = parse_tx_hashes(txs_19109955).unwrap();

        //let txs_send = parse_tx_hashes(txs_send_19101579).unwrap();

        /*for tx_hash in txs_send.iter() {
            match client_clone.get_transaction(*tx_hash).await {
                Ok(tx_option) => {
                    match tx_option {
                        Some(tx)=>{
                            match client_clone.send_raw_transaction(tx.rlp()).await {
                                Ok(p)=>{
                                    debug!("Transaction sent {}", p.tx_hash());
                                }
                                Err(e)=>{
                                    error!("Error sending transaction : {e}");
                                }
                            }
                            while client_clone.get_transaction_receipt(tx.hash).await.ok().is_none() {
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        }
                        None=>{
                            error!("Tx is none")
                        }
                    }
                }
                Err(e)=>{error!("Cannot get tx : {e}")}
            }

        }
        */

        //TODO : next_base_fee
        let next_block_base_fee = 1u128;

        if let Err(e) = market_events_channel_clone
            .send(MarketEvents::BlockHeaderUpdate {
                block_number: block_header.number.unwrap_or_default(),
                block_hash: block_header.hash.unwrap_or_default(),
                timestamp: block_header.timestamp,
                base_fee: block_header.base_fee_per_gas.unwrap_or_default(),
                next_base_fee: next_block_base_fee,
            })
            .await
        {
            error!("{}", e);
        }

        if let Err(e) = market_events_channel_clone
            .send(MarketEvents::BlockStateUpdate {
                block_hash: block_header.hash.unwrap_or_default(),
            })
            .await
        {
            error!("{}", e);
        }

        /*
        for tx_hash in txs_hash.iter() {
            match client_clone.get_transaction_by_hash(*tx_hash).await {
                Ok(tx_option) => {
                    match tx_option {
                        Some(tx) => {
                            info!("tx_found {:?}", tx);
                            {
                                let tx_hash: TxHash = tx.hash;
                                //let typed_tx: TypedTransaction = (&tx).into();
                                let mut mempool_guard = mempool_instance.write().await;
                                mempool_guard.add_tx(tx);
                                mempool_events_channel
                                    .send(MempoolEvents::MempoolActualTxUpdate { tx_hash })
                                    .await;
                                /*
                                match trace_call_diff(client_clone.clone(), typed_tx, Some(BlockNumber::Number(block_nr))).await {
                                    Ok(tx_diff)=> {
                                        info!("trace_call_diff {:?}", tx_diff);
                                        mempool_guard.add_tx_state_change(tx_hash, tx_diff);
                                        mempool_events_channel.send(MempoolEvents::MempoolStateUpdate(tx_hash)).await;
                                    }
                                    Err(e)=>{error!("{e}")}
                                }
                                 */
                            }

                            /*

                            let typed_tx : TypedTransaction = (&tx).into();
                            match client.send_raw_transaction(tx.rlp()).await {
                            //match client.trace_call(typed_tx, vec![TraceType::Trace], None).await {
                            //match client.debug_trace_call(typed_tx, None, TRACING_CALL_OPTS ).await {
                            //match debugtrace_call_diff(client.clone(), typed_tx, BlockNumber::Latest).await {
                                Ok(pending_tx)=> {
                                    info!("pending_tx {:?}", pending_tx);
                                }
                                Err(e)=>{error!("{e}")}
                            }

                             */
                        }
                        None => {
                            error!("tx not found {tx_hash}");
                        }
                    }
                }
                Err(e) => {
                    error!("Transaction not found {} : {}", tx_hash, e)
                }
            }
        }

         */

        tokio::time::sleep(Duration::from_millis(30000)).await;

        /*
        match client.dev_rpc().revert_to_snapshot(snap_id).await {
            Ok(..)=>{debug!("Reverted to snapshot {}", snap_id)}
            Err(e)=>{error!("{e}")}
        }

         */
    });

    println!("Hello, test is started!");

    let mut s = market_events_channel.clone().subscribe().await;
    loop {
        let msg = s.recv().await;
        match msg {
            Ok(msg) => match msg {
                MarketEvents::BlockTxUpdate {
                    block_number,
                    block_hash,
                } => {
                    info!("New block received {} {}", block_number, block_hash);
                }
                _ => {
                    debug!("event: {:?}", msg)
                }
            },
            Err(e) => {
                error!("{e}")
            }
        }
    }

    Ok(())
}
