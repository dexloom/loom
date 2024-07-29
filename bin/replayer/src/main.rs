use std::env;
use std::sync::Arc;
use std::time::Duration;

use alloy::primitives::{address, Address};
use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use eyre::Result;
use log::{debug, error, info};
use tokio::select;
use url::Url;

use debug_provider::HttpCachedTransport;
use defi_actors::{BlockchainActors, NodeBlockPlayerActor};
use defi_blockchain::Blockchain;
use defi_entities::{PoolClass, Swap, SwapAmountType, SwapLine};
use defi_events::{MessageTxCompose, TxComposeData};
use loom_utils::tokens::{USDC_ADDRESS, WETH_ADDRESS};
use loom_utils::NWETH;

#[tokio::main]
async fn main() -> Result<()> {
    let start_block_number = 20179184;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(
        "debug,alloy_rpc_client=off,debug_provider=debug,alloy_transport_http=off,hyper_util=off,defi_actors::block_history=debug",
    ))
    .format_timestamp_micros()
    .init();

    let node_url = env::var("MAINNET_HTTP")?;

    let transport = HttpCachedTransport::new(Url::parse(node_url.as_str())?, Some("./.cache")).await;
    transport.set_block_number(start_block_number);

    let client = ClientBuilder::default().transport(transport.clone(), true).with_poll_interval(Duration::from_millis(50));
    let provider = ProviderBuilder::new().on_client(client);

    // creating singers
    //let tx_signers = SharedState::new(TxSigners::new());

    // new blockchain
    let bc = Blockchain::new(1);

    const POOL_ADDRESS: Address = address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640");

    // instead fo code above
    let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone());
    bc_actors
        .with_nonce_and_balance_monitor_only_events()?
        .initialize_signers_with_anvil()?
        .with_market_state_preloader_virtual(vec![])?
        .with_pools_preloaded(vec![(POOL_ADDRESS, PoolClass::UniswapV3)])?
        .with_block_history()?
        .with_gas_station()?
        .with_swap_encoder(None)?
        .with_evm_estimator()?;

    //Start node block player actor
    if let Err(e) = bc_actors.start(NodeBlockPlayerActor::new(provider.clone(), start_block_number, start_block_number + 20).on_bc(&bc)) {
        panic!("Cannot start block player : {}", e);
    }

    tokio::task::spawn(bc_actors.wait());
    let compose_channel = bc.compose_channel();

    let mut header_sub = bc.new_block_headers_channel().subscribe().await;
    let mut block_sub = bc.new_block_with_tx_channel().subscribe().await;
    let mut logs_sub = bc.new_block_logs_channel().subscribe().await;
    let mut state_update_sub = bc.new_block_state_update_channel().subscribe().await;

    //let memepool = bc.mempool();
    let market = bc.market();
    let market_state = bc.market_state();

    let gas_station = bc.gas_station();

    loop {
        select! {
            header = header_sub.recv() => {
                match header{
                    Ok(header)=>{
                        info!("Block header received : {} {}", header.number.unwrap_or_default(), header.hash.unwrap_or_default());

                        if header.number.unwrap_or_default() % 10 == 0 {
                            let swap_path = market.read().await.swap_path(vec![WETH_ADDRESS, USDC_ADDRESS], vec![POOL_ADDRESS])?;
                            let mut swap_line = SwapLine::from(swap_path);
                            swap_line.amount_in = SwapAmountType::Set( NWETH::from_float(0.1));
                            swap_line.gas_used = Some(300000);

                            let tx_compose_encode_msg = MessageTxCompose::encode(
                                TxComposeData{
                                    gas_fee : gas_station.read().await.get_next_base_fee(),
                                    poststate : Some(Arc::new(market_state.read().await.state_db.clone())),
                                    swap : Swap::ExchangeSwapLine(swap_line),
                                    ..TxComposeData::default()
                                });

                            if let Err(e) = compose_channel.send(tx_compose_encode_msg).await {
                                error!("compose_channel.send : {}", e)
                            }else{
                                debug!("compose_channel.send ok");
                            }

                        }


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
