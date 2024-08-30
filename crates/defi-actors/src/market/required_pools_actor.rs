use std::marker::PhantomData;

use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use log::{debug, error};

use crate::fetch_and_add_pool_by_address;
use crate::market::fetch_state_and_add_pool;
use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::required_state::{RequiredState, RequiredStateReader};
use defi_entities::{Market, MarketState, PoolClass};
use defi_pools::protocols::CurveProtocol;
use defi_pools::CurvePool;
use loom_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

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
        match pool_class {
            PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
                debug!("Loading uniswap pool");
                fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), pool_address, pool_class).await?;
                debug!("Loaded uniswap pool ");
            }
            PoolClass::Curve => {
                debug!("Loading curve pool");
                if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(), pool_address).await {
                    let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
                    fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
                } else {
                    error!("CURVE_POOL_NOT_LOADED");
                }
                debug!("Loaded curve pool");
            }
            _ => {
                error!("Unknown pool class")
            }
        }
    }

    if let Some(required_state) = required_state {
        let update = RequiredStateReader::fetch_calls_and_slots(client.clone(), required_state, None).await?;
        market_state.write().await.state_db.apply_geth_update(update);
    }

    Ok("curve_protocol_loader_worker".to_string())
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
