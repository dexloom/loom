use std::env;
use std::fmt::{Display, Formatter};
use std::process::exit;
use std::time::Duration;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{Address, TxHash, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, BlockNumberOrTag, BlockTransactionsKind};
use clap::Parser;
use debug_provider::AnvilDebugProviderFactory;
use defi_actors::{
    fetch_and_add_pool_by_address, fetch_state_and_add_pool, AnvilBroadcastActor, ArbSwapPathMergerActor, BackrunConfig, BlockHistoryActor,
    DiffPathMergerActor, EvmEstimatorActor, InitializeSignersOneShotBlockingActor, MarketStatePreloadedOneShotActor, NodeBlockActor,
    NodeBlockActorConfig, NonceAndBalanceMonitorActor, PriceActor, SamePathMergerActor, StateChangeArbActor, SwapRouterActor,
    TxSignersActor,
};
use defi_entities::{AccountNonceAndBalanceState, BlockHistory, LatestBlock, Market, MarketState, PoolClass, Swap, Token, TxSigners};
use eyre::{OptionExt, Result};
use loom_utils::NWETH;
use tracing::{debug, error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

use defi_events::{
    MarketEvents, MempoolEvents, MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageHealthEvent,
    MessageTxCompose, TxCompose,
};
use defi_pools::protocols::CurveProtocol;
use defi_pools::CurvePool;
use defi_types::{debug_trace_block, ChainParameters, Mempool};
use loom_actors::{Accessor, Actor, Broadcaster, Consumer, Producer, SharedState};
use loom_multicaller::{MulticallerDeployer, MulticallerSwapEncoder};
use loom_revm_db::LoomDBType;

use crate::test_config::TestConfig;

mod default;
mod test_config;

#[derive(Clone, Default, Debug)]
struct Stat {
    found_counter: usize,
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
                        "Found: {} Ok: {} Profit : {} / ProfitEth : {} Path : {} ",
                        self.found_counter,
                        self.sign_counter,
                        token.to_float(swap.abs_profit()),
                        NWETH::to_float(swap.abs_profit_eth()),
                        swap
                    )
                }
                None => {
                    write!(
                        f,
                        "Found: {} Ok: {} Profit : {} / ProfitEth : {} Path : {} ",
                        self.found_counter,
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
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "debug,alloy_rpc_client=off".into());
    let fmt_layer = fmt::Layer::default().with_thread_ids(true).with_file(false).with_line_number(true).with_filter(env_filter);

    tracing_subscriber::registry().with(fmt_layer).init();

    let args = Commands::parse();

    let test_config = TestConfig::from_file(args.config.clone()).await?;

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

    let encoder = MulticallerSwapEncoder::new(multicaller_address);

    let block_nr = client.get_block_number().await?;
    info!("Block : {}", block_nr);

    let block_header = client.get_block(block_nr.into(), BlockTransactionsKind::Hashes).await?.unwrap().header;
    info!("Block header : {:?}", block_header);

    let block_header_with_txes = client.get_block(block_nr.into(), BlockTransactionsKind::Full).await?.unwrap();

    let cache_db = LoomDBType::default();
    let market_instance = Market::default();
    let market_state_instance = MarketState::new(cache_db.clone());

    let mempool_instance = Mempool::new();

    info!("Creating channels");
    let new_block_headers_channel: Broadcaster<MessageBlockHeader> = Broadcaster::new(10);
    let new_block_with_tx_channel: Broadcaster<MessageBlock> = Broadcaster::new(10);
    let new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate> = Broadcaster::new(10);
    let new_block_logs_channel: Broadcaster<MessageBlockLogs> = Broadcaster::new(10);

    //let new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate> = Broadcaster::new(500);

    let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(100);
    let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(500);
    let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(100);

    let market_instance = SharedState::new(market_instance);
    let market_state = SharedState::new(market_state_instance);
    let mempool_instance = SharedState::new(mempool_instance);
    let block_history_state = SharedState::new(BlockHistory::new(10));

    let tx_signers = TxSigners::new();
    let accounts_state = AccountNonceAndBalanceState::new();

    let tx_signers = SharedState::new(tx_signers);
    let accounts_state = SharedState::new(accounts_state);

    let latest_block = SharedState::new(LatestBlock::new(block_nr, block_header.hash));

    let (_, post) = debug_trace_block(client.clone(), BlockId::Number(BlockNumberOrTag::Number(block_nr)), true).await?;
    latest_block.write().await.update(
        block_nr,
        block_header.hash,
        Some(block_header.clone()),
        Some(block_header_with_txes),
        None,
        Some(post),
    );

    info!("Starting initialize signers actor");

    let mut initialize_signers_actor = InitializeSignersOneShotBlockingActor::new(Some(priv_key.to_bytes().to_vec()));
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
    let mut market_state_preload_actor = MarketStatePreloadedOneShotActor::new(client.clone())
        .with_copied_account(encoder.get_contract_address())
        .with_signers(tx_signers.clone());
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
    let mut node_block_actor = NodeBlockActor::new(client.clone(), NodeBlockActorConfig::all_enabled());
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

    for (pool_name, pool_config) in test_config.pools {
        match pool_config.class {
            PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
                debug!(address=%pool_config.address, class=%pool_config.class, "Loading pool");
                fetch_and_add_pool_by_address(
                    client.clone(),
                    market_instance.clone(),
                    market_state.clone(),
                    pool_config.address,
                    pool_config.class,
                )
                .await?;
                debug!(address=%pool_config.address, class=%pool_config.class, "Loaded pool");
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
        let swap_path_len = market_instance.read().await.get_pool_paths(&pool_config.address).unwrap_or_default().len();
        info!(
            "Loaded pool '{}' with address={}, pool_class={}, swap_paths={}",
            pool_name, pool_config.address, pool_config.class, swap_path_len
        );
    }

    info!("Starting block history actor");
    let mut block_history_actor = BlockHistoryActor::new(client.clone());
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

    // Start estimator actor
    let mut estimator_actor = EvmEstimatorActor::new_with_provider(encoder.clone(), Some(client.clone()));
    match estimator_actor.consume(tx_compose_channel.clone()).produce(tx_compose_channel.clone()).start() {
        Err(e) => error!("{e}"),
        _ => {
            info!("Estimate actor started successfully")
        }
    }

    // Start actor that encodes paths found
    if test_config.modules.encoder {
        info!("Starting swap router actor");

        let mut swap_router_actor = SwapRouterActor::new();

        match swap_router_actor
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
                info!("Swap router actor started successfully")
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
        let mut state_change_arb_actor = StateChangeArbActor::new(
            client.clone(),
            test_config.modules.arb_block,
            test_config.modules.arb_mempool,
            BackrunConfig::default(),
        );
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
            block_number: block_header.number,
            block_hash: block_header.hash,
            timestamp: block_header.timestamp,
            base_fee: block_header.base_fee_per_gas.unwrap_or_default(),
            next_base_fee: next_block_base_fee,
        })
        .await
    {
        error!("{}", e);
    }

    // Sending block state update message
    if let Err(e) = market_events_channel_clone.send(MarketEvents::BlockStateUpdate { block_hash: block_header.hash }).await {
        error!("{}", e);
    }

    //starting broadcasting transactions from eth to anvil
    let client_clone = client.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        info!("Re-broadcaster task started");

        for (_, tx_config) in test_config.txs.iter() {
            debug!("Fetching original tx {}", tx_config.hash);
            let Some(tx) = client_clone.get_transaction_by_hash(tx_config.hash).await.unwrap() else {
                panic!("Cannot get tx: {}", tx_config.hash);
            };

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
    });

    println!("Test '{}' is started!", args.config);

    let mut tx_compose_sub = tx_compose_channel.subscribe().await;

    let mut stat = Stat::default();
    let timeout_duration = Duration::from_secs(10);

    loop {
        tokio::select! {
            msg = tx_compose_sub.recv() => {
                match msg {
                    Ok(msg) => match msg.inner {
                        TxCompose::Sign(sign_message) => {
                            debug!(swap=%sign_message.swap, "Sign message");
                            stat.sign_counter += 1;

                            if stat.best_profit_eth < sign_message.swap.abs_profit_eth() {
                                stat.best_profit_eth = sign_message.swap.abs_profit_eth();
                                stat.best_swap = Some(sign_message.swap.clone());
                            }

                            if let Some(swaps_ok) = test_config.assertions.swaps_ok {
                                if stat.sign_counter >= swaps_ok  {
                                    break;
                                }
                            }
                        }
                        TxCompose::Route(encode_message) => {
                            debug!(swap=%encode_message.swap, "Route message");
                            stat.found_counter += 1;
                        }
                        _ => {}
                    },
                    Err(error) => {
                        error!(%error, "tx_compose_sub.recv")
                    }
                }
            }
            msg = tokio::time::sleep(timeout_duration) => {
                debug!(?msg, "Timed out");
                break;
            }
        }
    }

    println!("\n\n-------------------\nStat : {}\n-------------------\n", stat);

    if let Some(swaps_encoded) = test_config.assertions.swaps_encoded {
        if swaps_encoded > stat.found_counter {
            println!("Test failed. Not enough encoded swaps : {} need {}", stat.found_counter, swaps_encoded);
            exit(1)
        } else {
            println!("Test passed. Encoded swaps : {} required {}", stat.found_counter, swaps_encoded);
        }
    }
    if let Some(swaps_ok) = test_config.assertions.swaps_ok {
        if swaps_ok > stat.sign_counter {
            println!("Test failed. Not enough verified swaps : {} need {}", stat.sign_counter, swaps_ok);
            exit(1)
        } else {
            println!("Test passed. swaps : {} required {}", stat.sign_counter, swaps_ok);
        }
    }
    if let Some(best_profit) = test_config.assertions.best_profit_eth {
        if NWETH::from_float(best_profit) > stat.best_profit_eth {
            println!("Profit is too small {} need {}", NWETH::to_float(stat.best_profit_eth), best_profit);
            exit(1)
        } else {
            println!("Test passed. best profit : {} > {}", NWETH::to_float(stat.best_profit_eth), best_profit);
        }
    }

    Ok(())
}
