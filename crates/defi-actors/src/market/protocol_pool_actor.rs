use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use log::{error, info};

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState, PoolWrapper};
use defi_pools::protocols::CurveProtocol;
use defi_pools::CurvePool;
use loom_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

use crate::market::fetch_state_and_add_pool;

async fn curve_protocol_loader_worker<P, T, N>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
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
        if let Ok(curve_pool) = CurvePool::fetch_pool_data(client.clone(), curve_contract).await {
            let pool_wrapped = PoolWrapper::new(Arc::new(curve_pool));
            match fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), pool_wrapped.clone()).await {
                Err(e) => {
                    error!("Curve pool loading error : {}", e)
                }
                Ok(_) => {
                    info!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
                }
            }
        }
    }

    for factory_idx in 0..10 {
        if let Ok(factory_address) = CurveProtocol::get_factory_address(client.clone(), factory_idx).await {
            if let Ok(pool_count) = CurveProtocol::get_pool_count(client.clone(), factory_address).await {
                for pool_id in 0..pool_count {
                    if let Ok(addr) = CurveProtocol::get_pool_address(client.clone(), factory_address, pool_id).await {
                        if market.read().await.get_pool(&addr).is_some() {
                            continue;
                        }

                        match CurveProtocol::get_contract_from_code(client.clone(), addr).await {
                            Ok(curve_contract) => {
                                if let Ok(curve_pool) = CurvePool::fetch_pool_data(client.clone(), curve_contract).await {
                                    let pool_wrapped = PoolWrapper::new(Arc::new(curve_pool));

                                    match fetch_state_and_add_pool(
                                        client.clone(),
                                        market.clone(),
                                        market_state.clone(),
                                        pool_wrapped.clone(),
                                    )
                                    .await
                                    {
                                        Err(e) => {
                                            error!("Curve pool loading error {:?} : {}", pool_wrapped.get_address(), e);
                                        }
                                        Ok(_) => {
                                            info!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Contract from code error {:#20x} : {}", addr, e)
                            }
                        }
                    }
                }
            }
        }
    }

    Ok("curve_protocol_loader_worker".to_string())
}

#[derive(Accessor, Consumer)]
pub struct ProtocolPoolLoaderActor<P, T, N> {
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> ProtocolPoolLoaderActor<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, market: None, market_state: None, _n: PhantomData, _t: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), market_state: Some(bc.market_state()), ..self }
    }
}


impl<P, T, N> Actor for ProtocolPoolLoaderActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(curve_protocol_loader_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
        ));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "ProtocolPoolLoaderActor"
    }
}
