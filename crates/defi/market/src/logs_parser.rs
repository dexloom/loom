use alloy_network::{Ethereum, Network};
use alloy_primitives::Log as EVMLog;
use alloy_provider::Provider;
use alloy_rpc_types::Log;
use alloy_sol_types::SolEventInterface;
use alloy_transport::Transport;
use eyre::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

use loom_core_actors::{run_async, Broadcaster};
use loom_defi_abi::maverick::IMaverickPool::IMaverickPoolEvents;
use loom_defi_abi::uniswap2::IUniswapV2Pair::IUniswapV2PairEvents;
use loom_defi_abi::uniswap3::IUniswapV3Pool::IUniswapV3PoolEvents;
use loom_defi_pools::PoolsConfig;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{PoolClass, PoolLoaders};
use loom_types_events::Task;
//
// fn determine_pool_class(log_entry: Log) -> Option<PoolClass> {
//     {
//         let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
//         match log_entry {
//             Some(log_entry) => match IUniswapV3PoolEvents::decode_log(&log_entry, false) {
//                 Ok(event) => match event.data {
//                     IUniswapV3PoolEvents::Swap(_)
//                     | IUniswapV3PoolEvents::Mint(_)
//                     | IUniswapV3PoolEvents::Burn(_)
//                     | IUniswapV3PoolEvents::Initialize(_) => Some(PoolClass::UniswapV3),
//                     _ => None,
//                 },
//                 Err(_) => None,
//             }
//             .or_else(|| {
//                 {
//                     match IMaverickPoolEvents::decode_log(&log_entry, false) {
//                         Ok(event) => match event.data {
//                             IMaverickPoolEvents::Swap(_)
//                             | IMaverickPoolEvents::AddLiquidity(_)
//                             | IMaverickPoolEvents::RemoveLiquidity(_) => Some(PoolClass::UniswapV3),
//                             _ => None,
//                         },
//                         Err(_) => None,
//                     }
//                 }
//                 .or_else(|| match IUniswapV2PairEvents::decode_log(&log_entry, false) {
//                     Ok(event) => match event.data {
//                         IUniswapV2PairEvents::Swap(_)
//                         | IUniswapV2PairEvents::Mint(_)
//                         | IUniswapV2PairEvents::Burn(_)
//                         | IUniswapV2PairEvents::Sync(_) => Some(PoolClass::UniswapV2),
//                         _ => None,
//                     },
//                     Err(_) => None,
//                 })
//             }),
//             _ => None,
//         }
//     }
// }

pub async fn process_log_entries<P, T, N>(
    log_entries: Vec<Log>,
    pool_loaders: &PoolLoaders<P, T, N>,
    tasks_tx: Broadcaster<Task>,
) -> Result<()>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    let mut pool_to_fetch = Vec::new();
    let mut processed_pools = HashMap::new();

    for log_entry in log_entries.into_iter() {
        if let Some((pool_id, pool_class)) = pool_loaders.determine_pool_class(&log_entry) {
            // was this pool already processed?
            if processed_pools.insert(log_entry.address(), true).is_some() {
                continue;
            }

            pool_to_fetch.push((pool_id, pool_class));
        }
    }

    run_async!(tasks_tx.send(Task::FetchAndAddPools(pool_to_fetch)));
    Ok(())
}
