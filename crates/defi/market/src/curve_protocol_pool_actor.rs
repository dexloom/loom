use revm::{Database, DatabaseCommit};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use tracing::{debug, error};

use crate::pool_loader::fetch_state_and_add_pool;
use loom_core_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_defi_pools::protocols::CurveProtocol;
use loom_defi_pools::{CurvePool, CurvePoolAbiEncoder};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::{Market, MarketState, PoolId, PoolWrapper};
use revm::DatabaseRef;

async fn curve_pool_loader_worker<P, T, N, DB>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    let curve_contracts = CurveProtocol::get_contracts_vec(client.clone());
    for curve_contract in curve_contracts.into_iter() {
        if let Ok(curve_pool) =
            CurvePool::<P, T, N, CurvePoolAbiEncoder<P, T, N>>::fetch_pool_data_with_default_encoder(client.clone(), curve_contract).await
        {
            let pool_wrapped = PoolWrapper::new(Arc::new(curve_pool));
            match fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), pool_wrapped.clone()).await {
                Err(e) => {
                    error!("Curve pool loading error : {}", e)
                }
                Ok(_) => {
                    debug!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
                }
            }
        }
    }

    for factory_idx in 0..10 {
        if let Ok(factory_address) = CurveProtocol::get_factory_address(client.clone(), factory_idx).await {
            if let Ok(pool_count) = CurveProtocol::get_pool_count(client.clone(), factory_address).await {
                for pool_id in 0..pool_count {
                    if let Ok(addr) = CurveProtocol::get_pool_address(client.clone(), factory_address, pool_id).await {
                        if market.read().await.get_pool(&PoolId::Address(addr)).is_some() {
                            continue;
                        }

                        match CurveProtocol::get_contract_from_code(client.clone(), addr).await {
                            Ok(curve_contract) => {
                                if let Ok(curve_pool) =
                                    CurvePool::<P, T, N>::fetch_pool_data_with_default_encoder(client.clone(), curve_contract).await
                                {
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
                                            debug!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
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
pub struct CurvePoolLoaderOneShotActor<P, T, N, DB> {
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N, DB> CurvePoolLoaderOneShotActor<P, T, N, DB>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, market: None, market_state: None, _n: PhantomData, _t: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>) -> Self {
        Self { market: Some(bc.market()), market_state: Some(state.market_state_commit()), ..self }
    }
}

impl<P, T, N, DB> Actor for CurvePoolLoaderOneShotActor<P, T, N, DB>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(curve_pool_loader_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
        ));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "CurvePoolLoaderOneShotActor"
    }
}
