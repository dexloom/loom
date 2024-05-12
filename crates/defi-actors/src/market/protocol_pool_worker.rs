use std::fmt::Debug;
use std::sync::Arc;

use alloy_provider::Provider;
use async_trait::async_trait;
use log::{error, info};

use debug_provider::DebugProviderExt;
use defi_entities::{Market, MarketState, PoolWrapper};
use defi_pools::CurvePool;
use defi_pools::protocols::CurveProtocol;
use loom_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

use crate::market::fetch_and_add_pool;

async fn curve_protocol_loader_worker<P>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> WorkerResult
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    /*
    let steth_pool = StEthPool::new();

    match fetch_and_add_pool(client.clone(), market.clone(), market_state.clone(), steth_pool.clone()).await {
        Err(e) => {
            error!("StEth pool loading error : {}", e)
        }
        Ok(_) => {
            info!("StEth pool loaded {:#20x}", steth_pool.get_address());
        }
    }

    let wsteth_pool = WStEthPool::new();

    match fetch_and_add_pool(client.clone(), market.clone(), market_state.clone(), wsteth_pool.clone()).await {
        Err(e) => {
            error!("WstEth pool loading error : {}", e)
        }
        Ok(_) => {
            info!("WstEth pool loaded {:#20x}", wsteth_pool.get_address());
        }
    }
*/

    let curve_contracts = CurveProtocol::get_contracts_vec(client.clone());
    for curve_contract in curve_contracts.into_iter() {
        let curve_pool = CurvePool::from(curve_contract);
        let pool_wrapped = PoolWrapper::new(Arc::new(curve_pool));
        match fetch_and_add_pool(client.clone(), market.clone(), market_state.clone(), pool_wrapped.clone()).await {
            Err(e) => {
                error!("Curve pool loading error : {}", e)
            }
            Ok(_) => {
                info!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
            }
        }
    }


    for factory_idx in 0..10 {
        match CurveProtocol::get_factory_address(client.clone(), factory_idx).await {
            Ok(factory_address) => {
                match CurveProtocol::get_pool_count(client.clone(), factory_address).await {
                    Ok(pool_count) => {
                        for pool_id in 0..pool_count {
                            match CurveProtocol::get_pool_address(client.clone(), factory_address, pool_id).await {
                                Ok(addr) => {
                                    if market.read().await.get_pool(&addr).is_some() {
                                        continue;
                                    }

                                    match CurveProtocol::get_contract_from_code(client.clone(), addr).await {
                                        Ok(curve_contract) => {
                                            let curve_pool = CurvePool::from(curve_contract);
                                            let pool_wrapped = PoolWrapper::new(Arc::new(curve_pool));

                                            match fetch_and_add_pool(client.clone(), market.clone(), market_state.clone(), pool_wrapped.clone()).await {
                                                Err(e) => {
                                                    error!("Curve pool loading error {:?} : {}", pool_wrapped.get_address(), e);
                                                }
                                                Ok(_) => {
                                                    info!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("Contract from code error {:#20x} : {}", addr, e)
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok("curve_protocol_loader_worker".to_string())
}


#[derive(Accessor, Consumer)]
pub struct ProtocolPoolLoaderActor<P>
{
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
}

impl<P> ProtocolPoolLoaderActor<P>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            market: None,
            market_state: None,
        }
    }
}

#[async_trait]
impl<P> Actor for ProtocolPoolLoaderActor<P>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            curve_protocol_loader_worker(
                self.client.clone(),
                self.market.clone().unwrap(),
                self.market_state.clone().unwrap(),
            )
        );


        Ok(vec![task])
    }
}
