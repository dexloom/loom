use std::marker::PhantomData;

use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use tracing::{debug, error};

use crate::pool_loader::{fetch_and_add_pool_by_address, fetch_state_and_add_pool};
use loom_core_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::Blockchain;
use loom_defi_entities::required_state::{RequiredState, RequiredStateReader};
use loom_defi_entities::{Market, MarketState, PoolClass};
use loom_node_debug_provider::DebugProviderExt;
use loom_protocol_pools::protocols::CurveProtocol;
use loom_protocol_pools::CurvePool;

async fn required_pools_loader_worker<P, T, N>(
    client: P,
    pools: Vec<(Address, PoolClass)>,
    required_state: Option<RequiredState>,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    for (pool_address, pool_class) in pools {
        debug!(class=%pool_class, address=%pool_address, "Loading pool");
        match pool_class {
            PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
                if let Err(error) =
                    fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), pool_address, pool_class).await
                {
                    error!(%error, address = %pool_address, "fetch_and_add_pool_by_address")
                }
            }
            PoolClass::Curve => {
                if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(), pool_address).await {
                    let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
                    fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
                } else {
                    error!("CURVE_POOL_NOT_LOADED");
                }
            }
            _ => {
                error!("Unknown pool class")
            }
        }
        debug!(class=%pool_class, address=%pool_address, "Loaded pool");
    }

    if let Some(required_state) = required_state {
        let update = RequiredStateReader::fetch_calls_and_slots(client.clone(), required_state, None).await?;
        market_state.write().await.state_db.apply_geth_update(update);
    }

    Ok("required_pools_loader_worker".to_string())
}

#[derive(Accessor, Consumer)]
pub struct RequiredPoolLoaderActor<P, T, N> {
    client: P,
    pools: Vec<(Address, PoolClass)>,
    required_state: Option<RequiredState>,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> RequiredPoolLoaderActor<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, pools: Vec::new(), required_state: None, market: None, market_state: None, _n: PhantomData, _t: PhantomData }
    }

    pub fn with_pool(self, address: Address, pool_class: PoolClass) -> Self {
        let mut pools = self.pools;
        pools.push((address, pool_class));
        Self { pools, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), market_state: Some(bc.market_state()), ..self }
    }

    pub fn with_required_state(self, required_state: RequiredState) -> Self {
        Self { required_state: Some(required_state), ..self }
    }
}

impl<P, T, N> Actor for RequiredPoolLoaderActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(required_pools_loader_worker(
            self.client.clone(),
            self.pools.clone(),
            self.required_state.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
        ));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "RequiredPoolLoaderActor"
    }
}
