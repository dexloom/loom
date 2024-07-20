use std::env;
use std::fmt::{Display, Formatter};
use std::process::exit;
use std::time::Duration;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{Address, BlockHash, TxHash, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockId, BlockNumberOrTag, BlockTransactionsKind, Header};
use clap::Parser;
use env_logger::Env as EnvLog;
use eyre::{OptionExt, Result};
use log::{debug, error, info};

use debug_provider::AnvilDebugProviderFactory;
use defi_actors::{
    fetch_and_add_pool_by_address, fetch_state_and_add_pool, AnvilBroadcastActor, ArbSwapPathEncoderActor, ArbSwapPathMergerActor,
    BlockHistoryActor, DiffPathMergerActor, EvmEstimatorActor, GasStationActor, InitializeSignersActor, MarketStatePreloadedActor,
    NodeBlockActor, NonceAndBalanceMonitorActor, PriceActor, SamePathMergerActor, StateChangeArbActor, TxSignersActor,
};
use defi_entities::{
    AccountNonceAndBalanceState, BlockHistory, GasStation, LatestBlock, Market, MarketState, PoolClass, Swap, Token, TxSigners, NWETH,
};
use defi_events::{BlockLogs, BlockStateUpdate, MarketEvents, MempoolEvents, MessageHealthEvent, MessageTxCompose, TxCompose};
use defi_pools::protocols::CurveProtocol;
use defi_pools::CurvePool;
use defi_types::{debug_trace_block, ChainParameters, Mempool};
use loom_actors::{Accessor, Actor, Broadcaster, Consumer, Producer, SharedState};
use loom_multicaller::{MulticallerDeployer, SwapStepEncoder};
use loom_revm_db::LoomInMemoryDB;

use crate::test_config::TestConfig;

mod default;
mod test_config;

#[derive(Clone, Default, Debug)]
struct Stat {
    encode_counter: usize,
    sign_counter: usize,
    best_profit_eth: U256,
    best_swap: Option<Swap>,
}

impl Display for Stat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.best_swap {
            Some(swap) => match swap.get_first_token() {
                Some(token) => {
                    write!(
                        f,
                        "Encoded: {} Ok: {} Profit : {} / ProfitEth : {} Path : {} ",
                        self.encode_counter,
                        self.sign_counter,
                        token.to_float(swap.abs_profit()),
                        NWETH::to_float(swap.abs_profit_eth()),
                        swap
                    )
                }
                None => {
                    write!(
                        f,
                        "Encoded: {} Ok: {} Profit : {} / ProfitEth : {} Path : {} ",
                        self.encode_counter,
                        self.sign_counter,
                        swap.abs_profit(),
                        swap.abs_profit_eth(),
                        swap
                    )
                }
            },
            _ => {
                write!(f, "NO BEST SWAP")
            }
        }
    }
}

#[allow(dead_code)]
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
    env_logger::init_from_env(EnvLog::default().default_filter_or("debug,alloy_rpc_client=off"));

    let args = Commands::parse();

    let test_config = TestConfig::from_file(args.config).await?;

    let node_url = env::var("MAINNET_WS")?;

    let client = AnvilDebugProviderFactory::from_node_on_block(node_url, test_config.settings.block).await?;

    let priv_key = client.privkey()?;

    //let multicaller_address = MulticallerDeployer::new().deploy(client.clone(), priv_key.clone()).await?.address().ok_or_eyre("MULTICALLER_NOT_DEPLOYED")?;
    let multicaller_address = MulticallerDeployer::new()
        .set_code(client.clone(), Address::repeat_byte(0x78))
        .await?
        .address()
        .ok_or_eyre("MULTICALLER_NOT_DEPLOYED")?;
    info!("Multicaller deployed at {:?}", multicaller_address);

    let encoder = SwapStepEncoder::new(multicaller_address);

    let block_nr = client.get_block_number().await?;
    info!("Block : {}", block_nr);

    let block_header = client.get_block(block_nr.into(), BlockTransactionsKind::Hashes).await.unwrap().unwrap().header;

    let block_header_with_txes = client.get_block(block_nr.into(), BlockTransactionsKind::Full).await.unwrap().unwrap();

    let cache_db = LoomInMemoryDB::default();

    let market_instance = Market::default();

    let market_state_instance = MarketState::new(cache_db.clone());

    let mempool_instance = Mempool::new();

    info!("Creating channels");
    let new_block_headers_channel: Broadcaster<Header> = Broadcaster::new(10);
    let new_block_with_tx_channel: Broadcaster<Block> = Broadcaster::new(10);
    let new_block_state_update_channel: Broadcaster<BlockStateUpdate> = Broadcaster::new(10);
    let new_block_logs_channel: Broadcaster<BlockLogs> = Broadcaster::new(10);

    //let new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate> = Broadcaster::new(500);

    let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(100);
    let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(500);
    let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(100);

    let market_instance = SharedState::new(market_instance);

    let market_state = SharedState::new(market_state_instance);

    let mempool_instance = SharedState::new(mempool_instance);
    let gas_station_state = SharedState::new(GasStation::new());

    let block_history_state = SharedState::new(BlockHistory::fetch(client.clone(), market_state.inner(), 10).await?);

    let tx_signers = TxSigners::new();
    let accounts_state = AccountNonceAndBalanceState::new();

    let tx_signers = SharedState::new(tx_signers);
    let accounts_state = SharedState::new(accounts_state);

    let block_hash: BlockHash = block_header.hash.unwrap_or_default();

    let latest_block = SharedState::new(LatestBlock::new(block_nr, block_hash));

    let (_, post) = debug_trace_block(client.clone(), BlockId::Number(BlockNumberOrTag::Number(block_nr)), true).await?;
    latest_block.write().await.update(block_nr, block_hash, Some(block_header.clone()), Some(block_header_with_txes), None, Some(post));

    info!("Starting initialize signers actor");

    let mut initialize_signers_actor = InitializeSignersActor::new(Some(priv_key.to_bytes().to_vec()));
    match initialize_signers_actor.access(tx_signers.clone()).access(accounts_state.clone()).start_and_wait() {
        Err(e) => {
            error!("{}", e);
            panic!("Cannot initialize signers");
        }
        _ => info!("Signers have been initialized"),
    }

    for (token_name, token_config) in test_config.tokens {
        let symbol = token_config.symbol.unwrap_or(token_config.address.to_checksum(None));
        let name = token_config.name.unwrap_or(symbol.clone());
        let token = Token::new_with_data(
            token_config.address,
            Some(symbol),
            Some(name),
            Some(token_config.decimals.map_or(18, |x| x)),
            token_config.basic.unwrap_or_default(),
            token_config.middle.unwrap_or_default(),
        );
        if let Some(price_float) = token_config.price {
            let price_u256 = NWETH::from_float(price_float) * token.get_exp() / NWETH::get_exp();
            debug!("Setting price : {} -> {} ({})", token_name, price_u256, price_u256.to::<u128>());

            token.set_eth_price(Some(price_u256));
        };

        market_instance.write().await.add_token(token)?;
    }

    info!("Starting market state preload actor");
    let mut market_state_preload_actor =
        MarketStatePreloadedActor::new(client.clone()).with_encoder(&encoder).with_signers(tx_signers.clone());
    match market_state_preload_actor.access(market_state.clone()).start_and_wait() {
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
    {
        Err(e) => {
            error!("{}", e);
            panic!("Cannot initialize nonce and balance monitor");
        }
        _ => info!("Nonce monitor has been initialized"),
    }

    info!("Starting price actor");
    let mut price_actor = PriceActor::new(client.clone()).only_once();
    match price_actor.access(market_instance.clone()).start_and_wait() {
        Err(e) => {
            error!("{}", e);
            panic!("Cannot initialize price actor");
        }
        _ => info!("Price actor has been initialized"),
    }

    info!("Starting gas station actor");
    let mut gas_station_actor = GasStationActor::new();
    match gas_station_actor
        .access(gas_station_state.clone())
        .access(block_history_state.clone())
        .consume(market_events_channel.clone())
        .produce(market_events_channel.clone())
        .start()
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Gas station actor started successfully")
        }
    }

    for (_pool_name, pool_config) in test_config.pools {
        match pool_config.class {
            PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
                debug!("Loading uniswap pool");
                fetch_and_add_pool_by_address(
                    client.clone(),
                    market_instance.clone(),
                    market_state.clone(),
                    pool_config.address,
                    pool_config.class,
                )
                .await?;
                debug!("Loaded uniswap pool ");
            }
            PoolClass::Curve => {
                debug!("Loading curve pool");
                if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(), pool_config.address).await {
                    let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
                    fetch_state_and_add_pool(client.clone(), market_instance.clone(), market_state.clone(), curve_pool.into()).await?
                } else {
                    error!("CURVE_POOL_NOT_LOADED");
                }
                debug!("Loaded curve pool");
            }
            _ => {
                error!("Unknown pool class")
            }
        }
    }

    info!("Starting block history actor");
    let mut block_history_actor = BlockHistoryActor::new();
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
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Block history actor started successfully")
        }
    }

    let tx_compose_channel: Broadcaster<MessageTxCompose> = Broadcaster::new(100);

    let mut broadcast_actor = AnvilBroadcastActor::new(client.clone());
    match broadcast_actor.consume(tx_compose_channel.clone()).start() {
        Err(e) => error!("{}", e),
        _ => {
            info!("Broadcast actor started successfully")
        }
    }

    //let mut estimator_actor = HardhatEstimatorActor::new(client.clone(), encoder.clone());
    let mut estimator_actor = EvmEstimatorActor::new(encoder.clone());
    match estimator_actor.consume(tx_compose_channel.clone()).produce(tx_compose_channel.clone()).start() {
        Err(e) => error!("{e}"),
        _ => {
            info!("Estimate actor started successfully")
        }
    }

    // Start actor that encodes paths found
    if test_config.modules.encoder {
        info!("Starting swap path encoder actor");

        let mut swap_path_encoder_actor = ArbSwapPathEncoderActor::new(multicaller_address);

        match swap_path_encoder_actor
            .access(tx_signers.clone())
            .access(accounts_state.clone())
            .consume(tx_compose_channel.clone())
            .produce(tx_compose_channel.clone())
            .start()
        {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("Swap path encoder actor started successfully")
            }
        }
    }

    // Start signer actor that signs paths before broadcasting
    if test_config.modules.signer {
        info!("Starting signers actor");
        let mut signers_actor = TxSignersActor::new();
        match signers_actor.consume(tx_compose_channel.clone()).produce(tx_compose_channel.clone()).start() {
            Err(e) => {
                error!("{}", e);
                panic!("Cannot start signers");
            }
            _ => info!("Signers actor started"),
        }
    }

    //
    if test_config.modules.arb_block || test_config.modules.arb_mempool {
        info!("Starting state change arb actor");
        let mut state_change_arb_actor =
            StateChangeArbActor::new(client.clone(), test_config.modules.arb_block, test_config.modules.arb_mempool);
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
            .start()
        {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("State change arb actor started successfully")
            }
        }
    }

    // Swap path merger tries to build swap steps from swap lines
    if test_config.modules.arb_path_merger {
        info!("Starting swap path merger actor");
        let mut swap_path_merger_actor = ArbSwapPathMergerActor::new(multicaller_address);
        match swap_path_merger_actor
            .access(latest_block.clone())
            .consume(tx_compose_channel.clone())
            .consume(market_events_channel.clone())
            .produce(tx_compose_channel.clone())
            .start()
        {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("Swap path merger actor started successfully")
            }
        }
    }

    // Same path merger tries to merge different stuffing tx to optimize swap line
    if test_config.modules.same_path_merger {
        let mut same_path_merger_actor = SamePathMergerActor::new(client.clone());
        match same_path_merger_actor
            .access(market_state.clone())
            .access(latest_block.clone())
            .consume(tx_compose_channel.clone())
            .consume(market_events_channel.clone())
            .produce(tx_compose_channel.clone())
            .start()
        {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("Same path merger actor started successfully")
            }
        }
    }

    // Diff path merger tries to merge all found swaplines into one transaction s
    let mut diff_path_merger_actor = DiffPathMergerActor::new();
    match diff_path_merger_actor
        .consume(tx_compose_channel.clone())
        .consume(market_events_channel.clone())
        .produce(tx_compose_channel.clone())
        .start()
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Diff path merger actor started successfully")
        }
    }

    let next_block_base_fee = ChainParameters::ethereum().calc_next_block_base_fee(
        block_header.gas_used,
        block_header.gas_limit,
        block_header.base_fee_per_gas.unwrap_or_default(),
    );

    let market_events_channel_clone = market_events_channel.clone();

    // Sending block header update message
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

    // Sending block state update message
    if let Err(e) =
        market_events_channel_clone.send(MarketEvents::BlockStateUpdate { block_hash: block_header.hash.unwrap_or_default() }).await
    {
        error!("{}", e);
    }

    //starting broadcasting transactions from eth to anvil
    let client_clone = client.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        info!("Re-broadcaster task started");

        for (_, tx_config) in test_config.txs.iter() {
            debug!("Fetching original tx {}", tx_config.hash);
            match client_clone.get_transaction_by_hash(tx_config.hash).await {
                Ok(tx_option) => match tx_option {
                    Some(tx) => {
                        let from = tx.from;
                        let to = tx.to.unwrap_or_default();
                        if let Ok(tx_env) = TryInto::<TxEnvelope>::try_into(tx.clone()) {
                            match tx_config.send.to_lowercase().as_str() {
                                "mempool" => {
                                    let mut mempool_guard = mempool_instance.write().await;
                                    let tx_hash: TxHash = tx.hash;

                                    mempool_guard.add_tx(tx.clone());
                                    if let Err(e) = mempool_events_channel.send(MempoolEvents::MempoolActualTxUpdate { tx_hash }).await {
                                        error!("{e}");
                                    }
                                }
                                "block" => match client_clone.send_raw_transaction(tx_env.encoded_2718().as_slice()).await {
                                    Ok(p) => {
                                        debug!("Transaction sent {}", p.tx_hash());
                                    }
                                    Err(e) => {
                                        error!("Error sending transaction : {e}");
                                    }
                                },
                                _ => {
                                    debug!("Incorrect action {} for : hash {} from {} to {}  ", tx_config.send, tx_env.tx_hash(), from, to);
                                }
                            }
                        }
                    }
                    None => {
                        error!("Tx is none")
                    }
                },
                Err(e) => {
                    error!("Cannot get tx : {e}")
                }
            }
        }
    });

    println!("Test is started!");

    let mut s = tx_compose_channel.subscribe().await;

    let mut stat = Stat::default();
    let timeout_duration = Duration::from_secs(5);

    loop {
        tokio::select! {
            msg = s.recv() => {
                match msg {
                    Ok(msg) => match msg.inner {
                        TxCompose::Sign(sign_message) => {
                            debug!("Sign message. Swap : {}", sign_message.swap);
                            stat.sign_counter += 1;
                            if stat.best_profit_eth < sign_message.swap.abs_profit_eth() {
                                stat.best_profit_eth = sign_message.swap.abs_profit_eth();
                                stat.best_swap = Some(sign_message.swap.clone());
                            }
                        }
                        TxCompose::Encode(encode_message) => {
                            debug!("Encode message. Swap : {}", encode_message.swap);
                            stat.encode_counter +=1;
                        }
                        _ => {}
                    },
                    Err(e) => {
                        error!("{e}")
                    }
                }
            }
            msg = tokio::time::sleep(timeout_duration) => {
                debug!("Timed out : {:?} ", msg);
                break;
            }
        }
    }

    println!("\n\n-------------------\nStat : {}\n-------------------\n", stat);

    if let Some(results) = test_config.results {
        if let Some(swaps_encoded) = results.swaps_encoded {
            if swaps_encoded > stat.encode_counter {
                error!("Test failed. Not enough encoded swaps : {} need {}", stat.encode_counter, swaps_encoded);
                exit(1)
            }
        }
        if let Some(swaps_ok) = results.swaps_ok {
            if swaps_ok > stat.sign_counter {
                error!("Test failed. Not enough verified swaps : {} need {}", stat.sign_counter, swaps_ok);
                exit(1)
            }
        }
        if let Some(best_profit) = results.best_profit_eth {
            if NWETH::from_float(best_profit) > stat.best_profit_eth {
                error!("Profit is too small {} need {}", NWETH::to_float(stat.best_profit_eth), best_profit);
                exit(1)
            }
        }
    }

    Ok(())
}
