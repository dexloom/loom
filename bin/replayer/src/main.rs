use std::time::Duration;

use alloy::{
    providers::ProviderBuilder,
    rpc::client::ClientBuilder,
};
use eyre::Result;
use log::{error, info};
use tokio::select;
use url::Url;

use debug_provider::HttpCachedTransport;
use defi_actors::{BlockchainActors, NodeBlockPlayerActor};
use defi_blockchain::Blockchain;

#[tokio::main]
async fn main() -> Result<()> {
    let start_block_number = 20179184;
    let end_block_number = start_block_number + 1000;

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug,alloy_rpc_client=off,debug_provider=info,alloy_transport_http=off,hyper_util=off"));

    let transport = HttpCachedTransport::new(Url::parse("http://falcon.loop:8008/rpc")?, Some("./.cache")).await;
    transport.set_block_number(start_block_number);

    let client = ClientBuilder::default().transport(transport.clone(), true).with_poll_interval(Duration::from_millis(50));
    let provider = ProviderBuilder::new().on_client(client);

    // creating singers
    //let tx_signers = SharedState::new(TxSigners::new());

    // new blockchain
    let bc = Blockchain::new(1);


    /*
    let mut actor_manager = ActorsManager::new();

    // initializing signer
    if let Err(e) = actor_manager.start(InitializeSignersActor::new(None).with_signers(tx_signers.clone()).on_bc(&bc)).await {
        panic!("Cannot start signers : {}", e);
    }

    // starting singers actor
    if let Err(e) = actor_manager.start(SignersActor::new().on_bc(&bc)).await {
        panic!("Cannot start signers : {}", e);
    }


    // starting market state preloaded
    if let Err(e) = actor_manager.start(MarketStatePreloadedActor::new(provider.clone()).on_bc(&bc).with_signers(tx_signers)).await {
        panic!("Cannot start market state preloaded : {}", e);
    }

    // Start account nonce and balance monitor
    if let Err(e) = actor_manager.start(NonceAndBalanceMonitorActor::new(provider.clone()).on_bc(&bc)).await {
        panic!("Cannot start nonce and balance monitor : {}", e);
    }

    // Start block history actor
    if let Err(e) = actor_manager.start(BlockHistoryActor::new().on_bc(&bc)).await {
        panic!("Cannot start block history actor : {}", e);
    }

    // Start gas station actor
    if let Err(e) = actor_manager.start(GasStationActor::new().on_bc(&bc)).await {
        panic!("Cannot start gas station actor : {}", e);
    }
    */

    // instead fo code above
    let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone());
    bc_actors
        .initialize_signers_with_key(None).await?
        .with_market_state_preloader().await?
        .with_signers().await?
        .with_nonce_and_balance_monitor().await?
        .with_block_history().await?
        .with_gas_station().await?;


    // Start node block player actor
    if let Err(e) = bc_actors.start(NodeBlockPlayerActor::new(provider.clone(), start_block_number, end_block_number).on_bc(&bc)).await {
        panic!("Cannot start block player : {}", e);
    }


    tokio::task::spawn(bc_actors.wait());

    let mut header_sub = bc.new_block_headers_channel().subscribe().await;
    let mut block_sub = bc.new_block_with_tx_channel().subscribe().await;
    let mut logs_sub = bc.new_block_logs_channel().subscribe().await;
    let mut state_update_sub = bc.new_block_state_update_channel().subscribe().await;

    //let latest_block = bc.latest_block();
    let market_state = bc.market_state();

    loop {
        select! {
            header = header_sub.recv() => {
                match header{
                    Ok(header)=>{
                        info!("Block header received : {} {}", header.number.unwrap_or_default(), header.hash.unwrap_or_default());

                    }
                    Err(e)=>{
                        error!("Error receiving headers: {e}");
                    }
                }
            }

            logs = logs_sub.recv() => {
                match logs{
                    Ok(logs_update)=>{
                        info!("Block logs received : {} log records : {}", logs_update.block_hash, logs_update.logs.len());

                    }
                    Err(e)=>{
                        error!("Error receiving logs: {e}");
                    }
                }
            }

            block = block_sub.recv() => {
                match block {
                    Ok(block)=>{
                        info!("Block with tx received : {} txs : {}", block.header.hash.unwrap_or_default(), block.transactions.len());

                    }
                    Err(e)=>{
                        error!("Error receiving blocks: {e}");
                    }
                }
            }
            state_udpate = state_update_sub.recv() => {
                match state_udpate {
                    Ok(state_update)=>{
                        info!("Block state update received : {} update records : {}", state_update.block_hash, state_update.state_update.len() );
                        let state_db = market_state.read().await.state_db.clone();
                        info!("StateDB : Accounts: {} {} Contracts : {} {}", state_db.accounts.len(), state_db.db.accounts.len(), state_db.contracts.len(), state_db.db.contracts.len())

                    }
                    Err(e)=>{
                        error!("Error receiving blocks: {e}");
                    }
                }
            }

        }
    }
}
