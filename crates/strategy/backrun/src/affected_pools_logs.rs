use std::collections::BTreeMap;

use alloy_rpc_types::Log;
use alloy_sol_types::SolEventInterface;
use eyre::Result;
use loom_core_actors::SharedState;
use loom_defi_abi::uniswap4::IUniswapV4PoolManagerEvents::IUniswapV4PoolManagerEventsEvents;
use loom_defi_address_book::FactoryAddress;
use loom_types_entities::{Market, PoolId, PoolWrapper, SwapDirection};

#[allow(dead_code)]
pub async fn get_affected_pools_from_logs(
    market: SharedState<Market>,
    logs: &Vec<Log>,
) -> Result<BTreeMap<PoolWrapper, Vec<SwapDirection>>> {
    let market_guard = market.read().await;

    let mut affected_pools: BTreeMap<PoolWrapper, Vec<SwapDirection>> = BTreeMap::new();

    for log in logs.into_iter() {
        if log.address().eq(&FactoryAddress::UNISWAP_V4_POOL_MANAGER_ADDRESS) {
            if let Some(pool_id) = match IUniswapV4PoolManagerEventsEvents::decode_log(&log.inner, false) {
                Ok(event) => match event.data {
                    IUniswapV4PoolManagerEventsEvents::Initialize(params) => Some(params.id),
                    IUniswapV4PoolManagerEventsEvents::ModifyLiquidity(params) => Some(params.id),
                    IUniswapV4PoolManagerEventsEvents::Swap(params) => Some(params.id),
                    IUniswapV4PoolManagerEventsEvents::Donate(params) => Some(params.id),
                },
                Err(_) => None,
            } {
                if let Some(pool) = market_guard.get_pool(&PoolId::from(pool_id)) {
                    if !affected_pools.contains_key(pool) {
                        affected_pools.insert(pool.clone(), pool.get_swap_directions());
                    }
                }
            }
        }
    }

    Ok(affected_pools)
}
