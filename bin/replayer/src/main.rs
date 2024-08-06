use std::env;
use std::sync::Arc;
use std::time::Duration;

use alloy::contract::CallBuilder;
use alloy::primitives::{address, Address, U256};
use alloy::rpc::types::Header;
use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use eyre::Result;
use log::{debug, error, info};
use tokio::select;
use url::Url;

use debug_provider::HttpCachedTransport;
use defi_actors::{BlockchainActors, NodeBlockPlayerActor};
use defi_blockchain::Blockchain;
use defi_entities::required_state::RequiredState;
use defi_entities::{PoolClass, Swap, SwapAmountType, SwapLine};
use defi_events::{MessageTxCompose, TxComposeData};
use defi_pools::state_readers::ERC20StateReader;
use loom_multicaller::EncoderHelper;
use loom_utils::evm::env_for_block;
use loom_utils::tokens::{USDC_ADDRESS, WETH_ADDRESS};
use loom_utils::NWETH;

#[tokio::main]
async fn main() -> Result<()> {
    let start_block_number = 20179184;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(
        "debug,alloy_rpc_client=off,debug_provider=debug,alloy_transport_http=off,hyper_util=off,defi_actors::block_history=trace",
    ))
    .format_timestamp_micros()
    .init();

    let node_url = env::var("MAINNET_HTTP")?;
    let node_url = Url::parse(node_url.as_str())?;

    let transport = HttpCachedTransport::new(node_url.clone(), Some("./.cache")).await;
    transport.set_block_number(start_block_number);

    let client = ClientBuilder::default().transport(transport.clone(), true).with_poll_interval(Duration::from_millis(50));
    let provider = ProviderBuilder::new().on_client(client);

    let node_provider = ProviderBuilder::new().on_http(node_url);

    // creating singers
    //let tx_signers = SharedState::new(TxSigners::new());

    // new blockchain
    let bc = Blockchain::new(1);

    const POOL_ADDRESS: Address = address!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640");
    const TARGET_ADDRESS: Address = address!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C");

    let mut required_state = RequiredState::new();
    required_state.add_call(WETH_ADDRESS, EncoderHelper::encode_erc20_balance_of(TARGET_ADDRESS));

    // instead fo code above
    let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone());
    bc_actors
        .with_nonce_and_balance_monitor_only_events()?
        .initialize_signers_with_anvil()?
        .with_market_state_preloader_virtual(vec![])?
        .with_preloaded_state(vec![(POOL_ADDRESS, PoolClass::UniswapV3)], Some(required_state))?
        .with_block_history()?
        .with_gas_station()?
        .with_swap_encoder(None)?
        .with_evm_estimator()?;

    //Start node block player actor
    if let Err(e) = bc_actors.start(NodeBlockPlayerActor::new(provider.clone(), start_block_number, start_block_number + 200).on_bc(&bc)) {
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

    let mut cur_header: Header = Header::default();

    loop {
        select! {
            header = header_sub.recv() => {
                match header{
                    Ok(header)=>{
                        info!("Block header received : {} {}", header.number.unwrap_or_default(), header.hash.unwrap_or_default());
                        cur_header = header.clone();

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
                        let mut state_db = market_state.read().await.state_db.clone();
                        state_db.apply_geth_update_vec(state_update.state_update);

                        if let Ok(balance) = ERC20StateReader::balance_of(&state_db, env_for_block(cur_header.number.unwrap(), cur_header.timestamp), WETH_ADDRESS, TARGET_ADDRESS ) {
                            info!("------Balance of {} : {}", TARGET_ADDRESS, balance);
                            let fetched_balance = CallBuilder::new_raw(node_provider.clone(), EncoderHelper::encode_erc20_balance_of(TARGET_ADDRESS)).to(WETH_ADDRESS).block(cur_header.number.unwrap().into()).call().await?;

                            let fetched_balance = U256::from_be_slice(fetched_balance.to_vec().as_slice());
                            if fetched_balance != balance {
                                error!("Balance is wrong {:#x} need {:#x}", balance, fetched_balance);
                            }
                        }
                        if let Ok(balance) = ERC20StateReader::balance_of(&state_db, env_for_block(cur_header.number.unwrap(), cur_header.timestamp), WETH_ADDRESS, POOL_ADDRESS ) {
                            info!("------Balance of {} : {}", POOL_ADDRESS, balance);
                        }



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
