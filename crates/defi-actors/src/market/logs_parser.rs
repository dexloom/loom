use alloy_primitives::Log as EVMLog;
use alloy_rpc_types::Log;
use alloy_sol_types::SolEventInterface;
use eyre::Result;
use log::error;
use std::collections::HashMap;

use defi_abi::maverick::IMaverickPool::IMaverickPoolEvents;
use defi_abi::uniswap2::IUniswapV2Pair::IUniswapV2PairEvents;
use defi_abi::uniswap3::IUniswapV3Pool::IUniswapV3PoolEvents;
use defi_entities::PoolClass;
use defi_events::Task;
use defi_pools::PoolsConfig;
use loom_actors::{run_async, Broadcaster};

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

pub async fn process_log_entries(log_entries: Vec<Log>, pools_config: &PoolsConfig, tasks_tx: Broadcaster<Task>) -> Result<()> {
    let mut pool_to_fetch = Vec::new();
    let mut processed_pools = HashMap::new();

    for log_entry in log_entries.into_iter() {
        if let Some(pool_class) = determine_pool_class(log_entry.clone()) {
            if !pools_config.is_enabled(pool_class) {
                continue;
            }

            // was this pool already processed?
            if processed_pools.insert(log_entry.address(), true).is_some() {
                continue;
            }

            pool_to_fetch.push((log_entry.address(), pool_class));
        }
    }

    run_async!(tasks_tx.send(Task::FetchAndAddPools(pool_to_fetch)));
    Ok(())
}
