use alloy_network::Network;
use alloy_primitives::Address;
use alloy_primitives::Log as EVMLog;
use alloy_provider::Provider;
use alloy_rpc_types::Log;
use alloy_sol_types::SolEventInterface;
use alloy_transport::Transport;
use eyre::{eyre, Result};
use log::error;
use tokio::task::JoinHandle;

use debug_provider::DebugProviderExt;
use defi_abi::maverick::IMaverickPool::IMaverickPoolEvents;
use defi_abi::uniswap2::IUniswapV2Pair::IUniswapV2PairEvents;
use defi_abi::uniswap3::IUniswapV3Pool::IUniswapV3PoolEvents;
use defi_entities::{Market, MarketState, PoolClass};
use defi_pools::PoolsConfig;
use loom_actors::SharedState;

use super::pool_loader::fetch_and_add_pool_by_address;

fn determine_pool_class(log_entry: Log) -> Option<PoolClass> {
    {
        let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
        match log_entry {
            Some(log_entry) => match IUniswapV3PoolEvents::decode_log(&log_entry, false) {
                Ok(event) => match event.data {
                    IUniswapV3PoolEvents::Swap(_)
                    | IUniswapV3PoolEvents::Mint(_)
                    | IUniswapV3PoolEvents::Burn(_)
                    | IUniswapV3PoolEvents::Initialize(_) => Some(PoolClass::UniswapV3),
                    _ => None,
                },
                Err(_) => None,
            }
            .or_else(|| {
                {
                    match IMaverickPoolEvents::decode_log(&log_entry, false) {
                        Ok(event) => match event.data {
                            IMaverickPoolEvents::Swap(_)
                            | IMaverickPoolEvents::AddLiquidity(_)
                            | IMaverickPoolEvents::RemoveLiquidity(_) => Some(PoolClass::UniswapV3),
                            _ => None,
                        },
                        Err(_) => None,
                    }
                }
                .or_else(|| match IUniswapV2PairEvents::decode_log(&log_entry, false) {
                    Ok(event) => match event.data {
                        IUniswapV2PairEvents::Swap(_)
                        | IUniswapV2PairEvents::Mint(_)
                        | IUniswapV2PairEvents::Burn(_)
                        | IUniswapV2PairEvents::Sync(_) => Some(PoolClass::UniswapV2),
                        _ => None,
                    },
                    Err(_) => None,
                })
            }),
            _ => None,
        }
    }
}

pub async fn process_log_entries<P, T, N>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    log_entries: Vec<Log>,
    pools_config: &PoolsConfig,
) -> Result<Vec<Address>>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let mut tasks: Vec<JoinHandle<_>> = Vec::new();
    let mut pool_address_vec: Vec<Address> = Vec::new();

    for log_entry in log_entries.into_iter() {
        if let Some(pool_class) = determine_pool_class(log_entry.clone()) {
            if !pools_config.is_enabled(pool_class) {
                continue;
            }

            let mut market_guard = market.write().await;
            let market_pool = market_guard.is_pool(&log_entry.address());
            if !market_pool {
                {
                    if let Err(e) = market_guard.add_empty_pool(&log_entry.address()) {
                        error!("{}", e)
                    }
                }
                drop(market_guard);

                pool_address_vec.push(log_entry.address());

                tasks.push(tokio::task::spawn(fetch_and_add_pool_by_address(
                    client.clone(),
                    market.clone(),
                    market_state.clone(),
                    log_entry.address(),
                    pool_class,
                )));
            }
        }
    }

    for task in tasks {
        if let Err(e) = task.await {
            error!("Fetching pool task error : {}", e)
        }
    }
    if !pool_address_vec.is_empty() {
        Ok(pool_address_vec)
    } else {
        Err(eyre!("NO_POOLS_ADDED"))
    }
}
