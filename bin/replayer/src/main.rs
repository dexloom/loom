use std::env;
use std::sync::Arc;
use std::time::Duration;

use alloy::contract::CallBuilder;
use alloy::primitives::{address, Address, U256};
use alloy::rpc::types::Header;
use alloy::{providers::ProviderBuilder, rpc::client::ClientBuilder};
use eyre::Result;
use tokio::select;
use url::Url;

use debug_provider::HttpCachedTransport;
use defi_actors::{BlockchainActors, NodeBlockPlayerActor};
use defi_address_book::{TokenAddress, UniswapV3PoolAddress};
use defi_blockchain::Blockchain;
use defi_entities::required_state::RequiredState;
use defi_entities::{PoolClass, Swap, SwapAmountType, SwapLine};
use defi_events::{MessageTxCompose, TxComposeData};
use defi_pools::state_readers::ERC20StateReader;
use loom_multicaller::EncoderHelper;
use loom_utils::evm::env_for_block;
use loom_utils::NWETH;
use tracing::{debug, error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

#[tokio::main]
async fn main() -> Result<()> {
    let start_block_number = 20179184;
    // env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(
    //     "debug,alloy_rpc_client=off,debug_provider=info,alloy_transport_http=off,hyper_util=off,defi_actors::block_history=trace",
    // ))
    // .format_timestamp_micros()
    // .init();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        "debug,alloy_rpc_client=off,debug_provider=info,alloy_transport_http=off,hyper_util=off,defi_actors::block_history=trace".into()
    });
    let fmt_layer = fmt::Layer::default().with_thread_ids(true).with_file(false).with_line_number(true).with_filter(env_filter);

    tracing_subscriber::registry().with(fmt_layer).init();

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

    const TARGET_ADDRESS: Address = address!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C");

    let mut required_state = RequiredState::new();
    required_state.add_call(TokenAddress::WETH, EncoderHelper::encode_erc20_balance_of(TARGET_ADDRESS));

    // instead fo code above
    let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone(), vec![]);
    bc_actors
        .with_nonce_and_balance_monitor_only_events()?
        .initialize_signers_with_anvil()?
        .with_market_state_preloader_virtual(vec![])?
        .with_preloaded_state(vec![(UniswapV3PoolAddress::USDC_WETH_500, PoolClass::UniswapV3)], Some(required_state))?
        .with_block_history()?
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

    let mut cur_header: Header = Header::default();

    loop {
        select! {
            header = header_sub.recv() => {
                match header{
                    Ok(message_header)=>{
                        let header = message_header.inner.header;
                        info!("Block header received : {} {}", header.number, header.hash);
                        cur_header = header.clone();

                        if header.number % 10 == 0 {
                            let swap_path = market.read().await.swap_path(vec![TokenAddress::WETH, TokenAddress::USDC], vec![UniswapV3PoolAddress::USDC_WETH_500])?;
                            let mut swap_line = SwapLine::from(swap_path);
                            swap_line.amount_in = SwapAmountType::Set( NWETH::from_float(0.1));
                            swap_line.gas_used = Some(300000);

                            let tx_compose_encode_msg = MessageTxCompose::route(
                                TxComposeData{
                                    next_block_base_fee : bc.chain_parameters().calc_next_block_base_fee_from_header(&header),
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
                        info!("Block logs received : {} log records : {}", logs_update.block_header.hash, logs_update.logs.len());

                    }
                    Err(e)=>{
                        error!("Error receiving logs: {e}");
                    }
                }
            }

            block = block_sub.recv() => {
                match block {
                    Ok(block)=>{
                        info!("Block with tx received : {} txs : {}", block.header.hash, block.transactions.len());

                    }
                    Err(e)=>{
                        error!("Error receiving blocks: {e}");
                    }
                }
            }
            state_udpate = state_update_sub.recv() => {
                match state_udpate {
                    Ok(state_update)=>{
                        let state_update = state_update.inner;

                        info!("Block state update received : {} update records : {}", state_update.block_header.hash, state_update.state_update.len() );
                        let mut state_db = market_state.read().await.state_db.clone();
                        state_db.apply_geth_update_vec(state_update.state_update);

                        if let Ok(balance) = ERC20StateReader::balance_of(&state_db, env_for_block(cur_header.number, cur_header.timestamp), TokenAddress::WETH, TARGET_ADDRESS ) {
                            info!("------Balance of {} : {}", TARGET_ADDRESS, balance);
                            let fetched_balance = CallBuilder::new_raw(node_provider.clone(), EncoderHelper::encode_erc20_balance_of(TARGET_ADDRESS)).to(TokenAddress::WETH).block(cur_header.number.into()).call().await?;

                            let fetched_balance = U256::from_be_slice(fetched_balance.to_vec().as_slice());
                            if fetched_balance != balance {
                                error!("Balance is wrong {:#x} need {:#x}", balance, fetched_balance);
                            }
                        }
                        if let Ok(balance) = ERC20StateReader::balance_of(&state_db, env_for_block(cur_header.number, cur_header.timestamp), TokenAddress::WETH, UniswapV3PoolAddress::USDC_WETH_500 ) {
                            info!("------Balance of {} : {}", UniswapV3PoolAddress::USDC_WETH_500, balance);
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
