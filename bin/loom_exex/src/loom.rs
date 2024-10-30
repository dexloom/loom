use alloy::network::Ethereum;
use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::transports::Transport;
use axum::Router;
use debug_provider::DebugProviderExt;
use defi_actors::{loom_exex, BackrunConfig, BackrunConfigSection, NodeBlockActorConfig};
use defi_blockchain::Blockchain;
use defi_blockchain_actors::BlockchainActors;
use defi_entities::config::load_from_file;
use defi_entities::PoolClass;
use defi_pools::PoolsConfig;
use eyre::OptionExt;
use loom_db::init_db_pool;
use loom_test::SwapHealthMonitorActor;
use loom_topology::{BroadcasterConfig, EncoderConfig, TopologyConfig};
use reth_exex::ExExContext;
use reth_node_api::FullNodeComponents;
use std::env;
use std::future::Future;
use tracing::info;

pub async fn init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
    bc: Blockchain,
    config: NodeBlockActorConfig,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    Ok(loom_exex(ctx, bc, config.clone()))
}

pub async fn start_loom<P, T>(
    provider: P,
    bc: Blockchain,
    topology_config: TopologyConfig,
    loom_config_filepath: String,
    is_exex: bool,
) -> eyre::Result<()>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    let chain_id = provider.get_chain_id().await?;

    info!(chain_id = ?chain_id, "Starting Loom" );

    let (_encoder_name, encoder) = topology_config.encoders.iter().next().ok_or_eyre("NO_ENCODER")?;

    let multicaller_address: Option<Address> = match encoder {
        EncoderConfig::SwapStep(e) => e.address.parse().ok(),
    };
    let multicaller_address = multicaller_address.ok_or_eyre("MULTICALLER_ADDRESS_NOT_SET")?;
    let private_key_encrypted = hex::decode(env::var("DATA")?)?;
    info!(address=?multicaller_address, "Multicaller");

    let webserver_host = topology_config.webserver.unwrap_or_default().host;
    let db_url = topology_config.database.unwrap().url;
    let db_pool = init_db_pool(db_url).await?;

    // Get flashbots relays from config
    let relays = topology_config
        .actors
        .broadcaster
        .as_ref()
        .and_then(|b| b.get("mainnet"))
        .map(|b| match b {
            BroadcasterConfig::Flashbots(f) => f.relays(),
        })
        .unwrap_or_default();

    let pools_config = PoolsConfig::disable_all().enable(PoolClass::UniswapV2).enable(PoolClass::UniswapV3);

    let backrun_config: BackrunConfigSection = load_from_file::<BackrunConfigSection>(loom_config_filepath.into()).await?;
    let backrun_config: BackrunConfig = backrun_config.backrun_strategy;

    let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone(), relays);
    bc_actors
        .mempool()?
        .with_wait_for_node_sync()? // wait for node to sync before
        .initialize_signers_with_encrypted_key(private_key_encrypted)? // initialize signer with encrypted key
        .with_block_history()? // collect blocks
        .with_price_station()? // calculate price fo tokens
        .with_health_monitor_pools()? // monitor pools health to disable empty
        .with_health_monitor_state()? // monitor state health
        .with_health_monitor_stuffing_tx()? // collect stuffing tx information
        .with_swap_encoder(Some(multicaller_address))? // convert swaps to opcodes and passes to estimator
        .with_evm_estimator()? // estimate gas, add tips
        .with_signers()? // start signer actor that signs transactions before broadcasting
        .with_flashbots_broadcaster(false, true)? // broadcast signed txes to flashbots
        .with_market_state_preloader()? // preload contracts to market state
        .with_nonce_and_balance_monitor()? // start monitoring balances of
        .with_pool_history_loader(pools_config.clone())? // load pools used in latest 10000 blocks
        //.with_curve_pool_protocol_loader()? // load curve + steth + wsteth
        .with_new_pool_loader(pools_config.clone())? // load new pools
        .with_pool_loader()?
        .with_swap_path_merger()? // load merger for multiple swap paths
        .with_diff_path_merger()? // load merger for different swap paths
        .with_same_path_merger()? // load merger for same swap paths with different stuffing txes
        .with_backrun_block(backrun_config.clone())? // load backrun searcher for incoming block
        .with_backrun_mempool(backrun_config)? // load backrun searcher for mempool txes
        .with_web_server(webserver_host, Router::new(), db_pool)? // start web server
    ;

    if !is_exex {
        bc_actors.with_block_events(NodeBlockActorConfig::all_enabled())?.with_remote_mempool(provider.clone())?;
    }

    if let Some(influxdb_config) = topology_config.influxdb {
        bc_actors
            .with_influxdb_writer(influxdb_config.url, influxdb_config.database, influxdb_config.tags)?
            .with_block_latency_recorder()?;
    }
    if env::var("SWAP_HEALTH_MONITOR").unwrap_or_default() == "true" {
        bc_actors.start(SwapHealthMonitorActor::new(provider.clone()).on_bc(&bc))?;
    }
    bc_actors.wait().await;

    Ok(())
}
