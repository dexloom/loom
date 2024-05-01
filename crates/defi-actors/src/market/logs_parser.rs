use std::convert::Infallible;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::Address;
use alloy_primitives::Log as EVMLog;
use alloy_provider::Provider;
use alloy_rpc_types::Log;
use alloy_sol_types::SolEventInterface;
use eyre::{eyre, Result};
use log::error;
use tokio::task::JoinHandle;

use debug_provider::DebugProviderExt;
use defi_abi::uniswap2::IUniswapV2Pair::IUniswapV2PairEvents;
use defi_abi::uniswap3::IUniswapV3Pool::IUniswapV3PoolEvents;
use defi_entities::{Market, MarketState, PoolClass};
use loom_actors::SharedState;

use super::pool_loader::fetch_and_add_pool_by_address;

fn determine_pool_class(log_entry: Log) -> Option<PoolClass> {
    {
        let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
        match log_entry {
            Some(log_entry) => {
                match IUniswapV3PoolEvents::decode_log(&log_entry.clone(), false) {
                    Ok(event) => {
                        match event.data {
                            IUniswapV3PoolEvents::Swap(_) | IUniswapV3PoolEvents::Mint(_) | IUniswapV3PoolEvents::Burn(_) | IUniswapV3PoolEvents::Initialize(_) => {
                                Some(PoolClass::UniswapV3)
                            }
                            _ => None
                        }
                    }
                    Err(_) => None
                }
                    //TODO : maverick
                    /*.or_else(|| {
                    match parse_log::<MaverickPoolEvents>(log_entry.clone()) {
                        Ok(event) => {
                            match event {
                                MaverickPoolEvents::SwapFilter(_) | MaverickPoolEvents::AddLiquidityFilter(_) | MaverickPoolEvents::RemoveLiquidityFilter(_) => {
                                    Some(PoolClass::UniswapV3)
                                }
                                _ => None
                            }
                        }
                        Err(_) => None
                    }
                })*/.or_else(|| {
                    match IUniswapV2PairEvents::decode_log(&log_entry.clone().into(), false) {
                        Ok(event) => {
                            match event.data {
                                IUniswapV2PairEvents::Swap(_) | IUniswapV2PairEvents::Mint(_) | IUniswapV2PairEvents::Burn(_) | IUniswapV2PairEvents::Sync(_) => {
                                    Some(PoolClass::UniswapV2)
                                }
                                _ => None
                            }
                        }
                        Err(_) => {
                            None
                        }
                    }
                })/*.or_else(|| {
            match parse_log::<PancakeV3PoolEvents>(log_entry.clone()) {
                Ok(event) => {
                    match event {
                        PancakeV3PoolEvents::SwapFilter(_) | PancakeV3PoolEvents::MintFilter(_) | PancakeV3PoolEvents::BurnFilter(_) | PancakeV3PoolEvents::InitializeFilter(_) => {
                            Some(PoolClass::UniswapV3)
                        }
                        _ => None
                    }
                }
                Err(_) => None
            }
        })*/.or_else(|| None)
            }
            _ => None
        }
    }
}

pub async fn process_log_entries<P>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    log_entries: Vec<Log>,
) -> Result<Vec<Address>>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    let mut tasks: Vec<JoinHandle<_>> = Vec::new();
    let mut pool_address_vec: Vec<Address> = Vec::new();

    for log_entry in log_entries.into_iter() {
        match determine_pool_class(log_entry.clone()) {
            Some(pool_class) => {
                let mut market_guard = market.write().await;
                let market_pool = market_guard.is_pool(&log_entry.address());
                if !market_pool {
                    {
                        match market_guard.add_empty_pool(&log_entry.address()) {
                            Err(e) => error!("{}", e),
                            _ => {}
                        }
                    }
                    drop(market_guard);

                    pool_address_vec.push(log_entry.address());

                    tasks.push(tokio::task::spawn(
                        fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), log_entry.address(), pool_class)
                    ));
                }
            }
            _ => {}
        }
    }

    for task in tasks {
        match task.await {
            Err(e) => { error!("Fetching pool task error") }
            _ => {}
        }
    }

    Ok(pool_address_vec)
}
