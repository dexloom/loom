use std::env;
use std::fmt::{Display, Formatter};
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use alloy_rpc_types_eth::TransactionTrait;

use alloy_provider::network::TransactionResponse;

use crate::flashbots_mock::mount_flashbots_mock;
use crate::flashbots_mock::BundleRequest;
use crate::test_config::TestConfig;
use alloy_primitives::{address, TxHash, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, BlockNumberOrTag, BlockTransactionsKind};
use clap::Parser;
use loom::node::debug_provider::AnvilDebugProviderFactory;

use eyre::{ErrReport, OptionExt, Result};
use influxdb::WriteQuery;
use loom::broadcast::accounts::{InitializeSignersOneShotBlockingActor, NonceAndBalanceMonitorActor, TxSignersActor};
use loom::broadcast::broadcaster::{AnvilBroadcastActor, FlashbotsBroadcastActor};
use loom::broadcast::flashbots::client::RelayConfig;
use loom::broadcast::flashbots::Flashbots;
use loom::core::actors::{Accessor, Actor, Broadcaster, Consumer, Producer, SharedState};
use loom::core::block_history::BlockHistoryActor;
use loom::core::router::SwapRouterActor;
use loom::defi::address_book::TokenAddressEth;
use loom::defi::health_monitor::StuffingTxMonitorActor;
use loom::defi::market::{fetch_and_add_pool_by_pool_id, fetch_state_and_add_pool};
use loom::defi::pools::protocols::CurveProtocol;
use loom::defi::pools::{CurvePool, PoolLoadersBuilder, PoolsLoadingConfig};
use loom::defi::preloader::MarketStatePreloadedOneShotActor;
use loom::defi::price::PriceActor;
use loom::evm::db::LoomDBType;
use loom::evm::utils::evm_tx_env::env_from_signed_tx;
use loom::evm::utils::NWETH;
use loom::execution::estimator::EvmEstimatorActor;
use loom::execution::multicaller::{MulticallerDeployer, MulticallerSwapEncoder};
use loom::node::actor_config::NodeBlockActorConfig;
use loom::node::json_rpc::NodeBlockActor;
use loom::strategy::backrun::{BackrunConfig, StateChangeArbActor};
use loom::strategy::merger::{ArbSwapPathMergerActor, DiffPathMergerActor, SamePathMergerActor};
use loom::types::blockchain::{debug_trace_block, ChainParameters, LoomDataTypesEthereum, Mempool};
use loom::types::entities::{
    AccountNonceAndBalanceState, BlockHistory, LatestBlock, Market, MarketState, PoolClass, PoolId, Swap, Token, TxSigners,
};
use loom::types::events::{
    MarketEvents, MempoolEvents, MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageHealthEvent,
    MessageSwapCompose, MessageTxCompose, SwapComposeMessage,
};
use revm::db::EmptyDBTyped;
use tracing::{debug, error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};
use wiremock::MockServer;

mod flashbots_mock;
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

    /// Timout in seconds after the test fails
    #[arg(short, long, default_value = "10")]
    timeout: u64,

    /// Wait xx seconds before start re-broadcasting
    #[arg(short, long, default_value = "1")]
    wait_init: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "debug,alloy_rpc_client=off,loom_multicaller=trace".into());
    let fmt_layer = fmt::Layer::default().with_thread_ids(true).with_file(false).with_line_number(true).with_filter(env_filter);

    tracing_subscriber::registry().with(fmt_layer).init();

    let args = Commands::parse();
    let test_config = TestConfig::from_file(args.config.clone()).await?;
    let node_url = env::var("MAINNET_WS")?;
    let client = AnvilDebugProviderFactory::from_node_on_block(node_url, test_config.settings.block).await?;
    let priv_key = client.privkey()?.to_bytes().to_vec();

    let mut mock_server: Option<MockServer> = None;
    if test_config.modules.flashbots {
        // Start flashbots mock server
        mock_server = Some(MockServer::start().await);
        mount_flashbots_mock(mock_server.as_ref().unwrap()).await;
    }

    //let multicaller_address = MulticallerDeployer::new().deploy(client.clone(), priv_key.clone()).await?.address().ok_or_eyre("MULTICALLER_NOT_DEPLOYED")?;
    let multicaller_address = MulticallerDeployer::new()
        .set_code(client.clone(), address!("FCfCfcfC0AC30164AFdaB927F441F2401161F358"))
        .await?
        .address()
        .ok_or_eyre("MULTICALLER_NOT_DEPLOYED")?;
    info!("Multicaller deployed at {:?}", multicaller_address);

    let multicaller_encoder = MulticallerSwapEncoder::default_with_address(multicaller_address);

    let block_number = client.get_block_number().await?;
    info!("Current block_number={}", block_number);

    let block_header = client.get_block(block_number.into(), BlockTransactionsKind::Hashes).await?.unwrap().header;
    info!("Current block_header={:?}", block_header);

    let block_header_with_txes = client.get_block(block_number.into(), BlockTransactionsKind::Full).await?.unwrap();

    let cache_db = LoomDBType::default().with_ext_db(EmptyDBTyped::<ErrReport>::new());
    let mut market_instance = Market::default();
    let market_state_instance = MarketState::new(cache_db.clone());

    // Add default tokens for price actor
    let usdc_token = Token::new_with_data(TokenAddressEth::USDC, Some("USDC".to_string()), None, Some(6), true, false);
    let usdt_token = Token::new_with_data(TokenAddressEth::USDT, Some("USDT".to_string()), None, Some(6), true, false);
    let wbtc_token = Token::new_with_data(TokenAddressEth::WBTC, Some("WBTC".to_string()), None, Some(8), true, false);
    let dai_token = Token::new_with_data(TokenAddressEth::DAI, Some("DAI".to_string()), None, Some(18), true, false);
    market_instance.add_token(usdc_token)?;
    market_instance.add_token(usdt_token)?;
    market_instance.add_token(wbtc_token)?;
    market_instance.add_token(dai_token)?;

    let mempool_instance = Mempool::<LoomDataTypesEthereum>::new();

    info!("Creating channels");
    let new_block_headers_channel: Broadcaster<MessageBlockHeader> = Broadcaster::new(10);
    let new_block_with_tx_channel: Broadcaster<MessageBlock> = Broadcaster::new(10);
    let new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate> = Broadcaster::new(10);
    let new_block_logs_channel: Broadcaster<MessageBlockLogs> = Broadcaster::new(10);

    let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(100);
    let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(500);
    let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(100);

    let influx_channel: Broadcaster<WriteQuery> = Broadcaster::new(100);

    let market_instance = SharedState::new(market_instance);
    let market_state = SharedState::new(market_state_instance);
    let mempool_instance = SharedState::new(mempool_instance);
    let block_history_state = SharedState::new(BlockHistory::new(10));

    let tx_signers = TxSigners::new();
    let accounts_state = AccountNonceAndBalanceState::new();

    let tx_signers = SharedState::new(tx_signers);
    let accounts_state = SharedState::new(accounts_state);

    let latest_block = SharedState::new(LatestBlock::new(block_number, block_header.hash));

    let (_, post) = debug_trace_block(client.clone(), BlockId::Number(BlockNumberOrTag::Number(block_number)), true).await?;
    latest_block.write().await.update(
        block_number,
        block_header.hash,
        Some(block_header.clone()),
        Some(block_header_with_txes),
        None,
        Some(post),
    );

    info!("Starting initialize signers actor");

    let mut initialize_signers_actor = InitializeSignersOneShotBlockingActor::new(Some(priv_key));
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
        .with_copied_account(multicaller_encoder.get_contract_address())
        .with_signers(tx_signers.clone());
    match market_state_preload_actor.access(market_state.clone()).start_and_wait() {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Market state preload actor started successfully")
        }
    }

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
        .access(latest_block.clone())
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

    let pool_loaders = Arc::new(PoolLoadersBuilder::default_pool_loaders(client.clone(), PoolsLoadingConfig::default()));

    for (pool_name, pool_config) in test_config.pools {
        match pool_config.class {
            PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
                debug!(address=%pool_config.address, class=%pool_config.class, "Loading pool");
                fetch_and_add_pool_by_pool_id(
                    client.clone(),
                    market_instance.clone(),
                    market_state.clone(),
                    pool_loaders.clone(),
                    PoolId::Address(pool_config.address),
                    pool_config.class,
                )
                .await?;
                debug!(address=%pool_config.address, class=%pool_config.class, "Loaded pool");
            }
            PoolClass::Curve => {
                debug!("Loading curve pool");
                if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(), pool_config.address).await {
                    let curve_pool = CurvePool::fetch_pool_data_with_default_encoder(client.clone(), curve_contract).await?;
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
        let swap_path_len = market_instance.read().await.get_pool_paths(&PoolId::Address(pool_config.address)).unwrap_or_default().len();
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

    let swap_compose_channel: Broadcaster<MessageSwapCompose<LoomDBType>> = Broadcaster::new(100);
    let tx_compose_channel: Broadcaster<MessageTxCompose> = Broadcaster::new(100);

    let mut broadcast_actor = AnvilBroadcastActor::new(client.clone());
    match broadcast_actor.consume(tx_compose_channel.clone()).start() {
        Err(e) => error!("{}", e),
        _ => {
            info!("Broadcast actor started successfully")
        }
    }

    // Start estimator actor
    let mut estimator_actor = EvmEstimatorActor::new_with_provider(multicaller_encoder.clone(), Some(client.clone()));
    match estimator_actor.consume(swap_compose_channel.clone()).produce(swap_compose_channel.clone()).start() {
        Err(e) => error!("{e}"),
        _ => {
            info!("Estimate actor started successfully")
        }
    }

    let mut health_monitor_actor = StuffingTxMonitorActor::new(client.clone());
    match health_monitor_actor
        .access(latest_block.clone())
        .consume(market_events_channel.clone())
        .consume(tx_compose_channel.clone())
        .produce(influx_channel.clone())
        .start()
    {
        Ok(_) => {
            //tasks.extend(r);
            info!("Stuffing tx monitor actor started")
        }
        Err(e) => {
            panic!("StuffingTxMonitorActor error {}", e)
        }
    }

    // Start actor that encodes paths found
    if test_config.modules.encoder {
        info!("Starting swap router actor");

        let mut swap_router_actor = SwapRouterActor::new();

        match swap_router_actor
            .access(tx_signers.clone())
            .access(accounts_state.clone())
            .consume(swap_compose_channel.clone())
            .produce(swap_compose_channel.clone())
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

    // Start state change arb actor
    if test_config.modules.arb_block || test_config.modules.arb_mempool {
        info!("Starting state change arb actor");
        let mut state_change_arb_actor = StateChangeArbActor::new(
            client.clone(),
            test_config.modules.arb_block,
            test_config.modules.arb_mempool,
            BackrunConfig::new_dumb(),
        );
        match state_change_arb_actor
            .access(mempool_instance.clone())
            .access(latest_block.clone())
            .access(market_instance.clone())
            .access(market_state.clone())
            .access(block_history_state.clone())
            .consume(market_events_channel.clone())
            .consume(mempool_events_channel.clone())
            .produce(swap_compose_channel.clone())
            .produce(pool_health_monitor_channel.clone())
            .produce(influx_channel.clone())
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
            .consume(swap_compose_channel.clone())
            .consume(market_events_channel.clone())
            .produce(swap_compose_channel.clone())
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
            .consume(swap_compose_channel.clone())
            .consume(market_events_channel.clone())
            .produce(swap_compose_channel.clone())
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
    if test_config.modules.flashbots {
        let relays = vec![RelayConfig { id: 1, url: mock_server.as_ref().unwrap().uri(), name: "relay".to_string(), no_sign: Some(false) }];
        let flashbots = Flashbots::new(client.clone(), "https://unused", None).with_relays(relays);
        let mut flashbots_broadcast_actor = FlashbotsBroadcastActor::new(flashbots, true);
        match flashbots_broadcast_actor.consume(tx_compose_channel.clone()).start() {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("Flashbots broadcast actor started successfully")
            }
        }
    }

    // Diff path merger tries to merge all found swaplines into one transaction s
    let mut diff_path_merger_actor = DiffPathMergerActor::new();
    match diff_path_merger_actor
        .consume(swap_compose_channel.clone())
        .consume(market_events_channel.clone())
        .produce(swap_compose_channel.clone())
        .start()
    {
        Err(e) => {
            error!("{}", e)
        }
        _ => {
            info!("Diff path merger actor started successfully")
        }
    }

    // #### Blockchain events
    // we need to wait for all actors to start. For the CI it can be a bit longer
    tokio::time::sleep(Duration::from_secs(args.wait_init)).await;

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

    // #### RE-BROADCASTER
    //starting broadcasting transactions from eth to anvil
    let client_clone = client.clone();
    tokio::spawn(async move {
        info!("Re-broadcaster task started");

        for (_, tx_config) in test_config.txs.iter() {
            debug!("Fetching original tx {}", tx_config.hash);
            let Some(tx) = client_clone.get_transaction_by_hash(tx_config.hash).await.unwrap() else {
                panic!("Cannot get tx: {}", tx_config.hash);
            };

            let from = tx.from;
            let to = tx.to().unwrap_or_default();

            match tx_config.send.to_lowercase().as_str() {
                "mempool" => {
                    let mut mempool_guard = mempool_instance.write().await;
                    let tx_hash: TxHash = tx.tx_hash();

                    mempool_guard.add_tx(tx.clone());
                    if let Err(e) = mempool_events_channel.send(MempoolEvents::MempoolActualTxUpdate { tx_hash }).await {
                        error!("{e}");
                    }
                }
                "block" => match client_clone.send_raw_transaction(tx.inner.encoded_2718().as_slice()).await {
                    Ok(p) => {
                        debug!("Transaction sent {}", p.tx_hash());
                    }
                    Err(e) => {
                        error!("Error sending transaction : {e}");
                    }
                },
                _ => {
                    debug!("Incorrect action {} for : hash {} from {} to {}  ", tx_config.send, tx.tx_hash(), from, to);
                }
            }
        }
    });

    println!("Test '{}' is started!", args.config);

    let mut tx_compose_sub = swap_compose_channel.subscribe().await;

    let mut stat = Stat::default();
    let timeout_duration = Duration::from_secs(args.timeout);

    loop {
        tokio::select! {
            msg = tx_compose_sub.recv() => {
                match msg {
                    Ok(msg) => match msg.inner {
                        SwapComposeMessage::Ready(ready_message) => {
                            debug!(swap=%ready_message.swap, "Ready message");
                            stat.sign_counter += 1;

                            if stat.best_profit_eth < ready_message.swap.abs_profit_eth() {
                                stat.best_profit_eth = ready_message.swap.abs_profit_eth();
                                stat.best_swap = Some(ready_message.swap.clone());
                            }

                            if let Some(swaps_ok) = test_config.assertions.swaps_ok {
                                if stat.sign_counter >= swaps_ok  {
                                    break;
                                }
                            }
                        }
                        SwapComposeMessage::Prepare(encode_message) => {
                            debug!(swap=%encode_message.swap, "Prepare message");
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
    if test_config.modules.flashbots {
        // wait for flashbots mock server to receive all requests
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Some(last_requests) = mock_server.unwrap().received_requests().await {
            if last_requests.is_empty() {
                println!("Mock server did not received any request!")
            } else {
                println!("Received {} flashbots requests", last_requests.len());
                for request in last_requests {
                    let bundle_request: BundleRequest = serde_json::from_slice(&request.body)?;
                    println!(
                        "bundle_count={}, target_blocks={:?}, txs_in_bundles={:?}",
                        bundle_request.params.len(),
                        bundle_request.params.iter().map(|b| b.target_block).collect::<Vec<_>>(),
                        bundle_request.params.iter().map(|b| b.transactions.len()).collect::<Vec<_>>()
                    );
                    // print all transactions
                    for bundle in bundle_request.params {
                        for tx in bundle.transactions {
                            let tx_env = env_from_signed_tx(tx)?;
                            println!("tx={:?}", tx_env);
                        }
                    }
                }
            }
        } else {
            println!("Mock server did not received any request!")
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
