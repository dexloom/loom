use alloy_eips::BlockNumberOrTag;
use alloy_network::Network;
use alloy_provider::Provider;
use eyre::eyre;
use revm::primitives::Env;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{debug, error};

use loom_core_actors::SharedState;
use loom_defi_pools::protocols::{UniswapV2Protocol, UniswapV3Protocol};
use loom_defi_pools::state_readers::UniswapV3StateReader;
use loom_defi_pools::{MaverickPool, PancakeV3Pool, UniswapV2Pool, UniswapV3Pool};
use loom_evm_db::{AlloyDB, LoomDB};
use loom_types_blockchain::GethStateUpdateVec;
use loom_types_entities::{get_protocol_by_factory, Market, MarketState, Pool, PoolId, PoolProtocol, PoolWrapper, SwapDirection};

pub async fn get_affected_pools_from_code<P, N>(
    client: P,
    market: SharedState<Market>,
    state_update: &GethStateUpdateVec,
) -> eyre::Result<BTreeMap<PoolWrapper, Vec<SwapDirection>>>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    let mut market_state = MarketState::new(LoomDB::new());

    market_state.state_db.apply_geth_state_update(state_update, true, false);

    let mut ret: BTreeMap<PoolWrapper, Vec<SwapDirection>> = BTreeMap::new();

    for state_update_record in state_update.iter() {
        for (address, state_update_entry) in state_update_record.iter() {
            if let Some(code) = &state_update_entry.code {
                if UniswapV2Protocol::is_code(code) {
                    match market.read().await.get_pool(&PoolId::Address(*address)) {
                        None => {
                            debug!(?address, "Loading UniswapV2 class pool");
                            let env = Env::default();

                            let ext_db = AlloyDB::new(client.clone(), BlockNumberOrTag::Latest.into());

                            let Some(ext_db) = ext_db else {
                                error!("Cannot create AlloyDB");
                                continue;
                            };

                            let state_db = market_state.state_db.clone().with_ext_db(ext_db);

                            match UniswapV3StateReader::factory(&state_db, env.clone(), *address) {
                                Ok(_factory_address) => match UniswapV2Pool::fetch_pool_data_evm(&state_db, env.clone(), *address) {
                                    Ok(pool) => {
                                        let pool = PoolWrapper::new(Arc::new(pool));
                                        let protocol = pool.get_protocol();
                                        let swap_directions = pool.get_swap_directions();

                                        debug!(%address, %protocol, ?swap_directions, "UniswapV2 pool loaded");
                                        ret.insert(pool, swap_directions);
                                    }
                                    Err(err) => {
                                        error!(?address, %err, "Error loading UniswapV2 pool");
                                    }
                                },
                                Err(err) => {
                                    error!(?address, %err, "Error loading UniswapV2 factory for pool")
                                }
                            }
                        }
                        Some(pool) => {
                            debug!(?address, protocol = ?pool.get_protocol(), "Pool already exists");
                        }
                    }
                }

                if UniswapV3Protocol::is_code(code) {
                    match market.read().await.get_pool(&PoolId::Address(*address)) {
                        None => {
                            debug!(%address, "Loading UniswapV3 class pool");
                            let env = Env::default();

                            let ext_db = AlloyDB::new(client.clone(), BlockNumberOrTag::Latest.into());

                            let Some(ext_db) = ext_db else {
                                error!("Cannot create AlloyDB");
                                continue;
                            };

                            let state_db = market_state.state_db.clone().with_ext_db(ext_db);

                            match UniswapV3StateReader::factory(&state_db, env.clone(), *address) {
                                Ok(factory_address) => {
                                    match get_protocol_by_factory(factory_address) {
                                        PoolProtocol::PancakeV3 => {
                                            let pool = PancakeV3Pool::fetch_pool_data_evm(&state_db, env.clone(), *address);
                                            match pool {
                                                Ok(pool) => {
                                                    let swap_directions = pool.get_swap_directions();
                                                    let protocol = pool.get_protocol();
                                                    debug!(?address, %protocol, ?swap_directions, "PancakeV3 Pool loaded");
                                                    ret.insert(PoolWrapper::new(Arc::new(pool)), swap_directions);
                                                }
                                                Err(err) => {
                                                    error!(?address, %err, "Error loading PancakeV3 pool");
                                                }
                                            }
                                        }
                                        PoolProtocol::Maverick => {
                                            let pool = MaverickPool::fetch_pool_data_evm(&state_db, env.clone(), *address);
                                            match pool {
                                                Ok(pool) => {
                                                    let pool = PoolWrapper::new(Arc::new(pool));
                                                    let swap_directions = pool.get_swap_directions();
                                                    let protocol = pool.get_protocol();
                                                    debug!(?address, %protocol, ?swap_directions, "Maverick Pool loaded");

                                                    ret.insert(pool, swap_directions);
                                                }
                                                Err(err) => {
                                                    error!(?address, %err, "Error loading Maverick pool");
                                                }
                                            }
                                        }
                                        _ => match UniswapV3Pool::fetch_pool_data_evm(&state_db, env.clone(), *address) {
                                            Ok(pool) => {
                                                let pool = PoolWrapper::new(Arc::new(pool));
                                                let swap_directions = pool.get_swap_directions();
                                                let protocol = pool.get_protocol();
                                                debug!(
                                                    %address,
                                                    %protocol,
                                                    ?swap_directions,
                                                    "UniswapV3 Pool loaded"
                                                );
                                                ret.insert(pool, swap_directions);
                                            }
                                            Err(err) => {
                                                error!(%address, %err, "Error loading UniswapV3 pool");
                                            }
                                        },
                                    };
                                }
                                Err(err) => {
                                    error!(?address, %err, "Error loading UniswapV3 factory for pool")
                                }
                            }
                        }
                        Some(pool) => {
                            debug!(?address, protocol = ?pool.get_protocol(), "Pool already exists")
                        }
                    }
                }
            }
        }
    }
    if !ret.is_empty() {
        Ok(ret)
    } else {
        Err(eyre!("NO_POOLS_LOADED"))
    }
}

/// Check if the state update code contains code for a UniswapV2 pair or UniswapV3 pool by looking for method signatures.
pub fn is_pool_code(state_update: &GethStateUpdateVec) -> bool {
    for state_update_record in state_update.iter() {
        for (_address, state_update_entry) in state_update_record.iter() {
            if let Some(code) = &state_update_entry.code {
                if UniswapV3Protocol::is_code(code) {
                    return true;
                }
                if UniswapV2Protocol::is_code(code) {
                    return true;
                }
            }
        }
    }
    false
}
