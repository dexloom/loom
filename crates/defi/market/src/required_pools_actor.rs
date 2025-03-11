use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use revm::DatabaseRef;
use revm::{Database, DatabaseCommit};
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::pool_loader_actor::fetch_and_add_pool_by_pool_id;
use loom_core_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_evm_db::LoomDBError;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEVM};
use loom_types_entities::required_state::{RequiredState, RequiredStateReader};
use loom_types_entities::{EntityAddress, Market, MarketState, PoolClass, PoolLoaders};

async fn required_pools_loader_worker<P, N, DB, LDT>(
    client: P,
    pool_loaders: Arc<PoolLoaders<P, N, LDT>>,
    pools: Vec<(EntityAddress, PoolClass)>,
    required_state: Option<RequiredState>,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
) -> WorkerResult
where
    N: Network<TransactionRequest = LDT::TransactionRequest>,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database<Error = LoomDBError> + DatabaseRef<Error = LoomDBError> + DatabaseCommit + Send + Sync + Clone + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    for (pool_id, pool_class) in pools {
        debug!(class=%pool_class, %pool_id, "Loading pool");
        match fetch_and_add_pool_by_pool_id(client.clone(), market.clone(), market_state.clone(), pool_loaders.clone(), pool_id, pool_class)
            .await
        {
            Ok(_) => {
                info!(class=%pool_class, %pool_id, "pool loaded")
            }
            Err(error) => {
                error!(%error, "load_pool_with_provider")
            }
        }
    }
    //
    //
    //     match pool_class {
    //         PoolClass::UniswapV2 | PoolClass::UniswapV3 => {
    //             if let Err(error) =
    //                 fetch_and_add_pool_by_pool_id(client.clone(), market.clone(), market_state.clone(), pool_address, pool_class).await
    //             {
    //                 error!(%error, address = %pool_address, "fetch_and_add_pool_by_address")
    //             }
    //         }
    //         PoolClass::Curve => {
    //             if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(), pool_address).await {
    //                 let curve_pool = CurvePool::<P, T, N>::fetch_pool_data_with_default_encoder(client.clone(), curve_contract).await?;
    //                 fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
    //             } else {
    //                 error!("CURVE_POOL_NOT_LOADED");
    //             }
    //         }
    //         _ => {
    //             error!("Unknown pool class")
    //         }
    //     }
    //     debug!(class=%pool_class, address=%pool_address, "Loaded pool");
    // }

    if let Some(required_state) = required_state {
        let update = RequiredStateReader::<LDT>::fetch_calls_and_slots(client.clone(), required_state, None).await?;
        market_state.write().await.apply_geth_update(update);
    }

    Ok("required_pools_loader_worker".to_string())
}

#[derive(Accessor, Consumer)]
pub struct RequiredPoolLoaderActor<P, N, DB, LDT>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Clone + Send + Sync + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    client: P,
    pool_loaders: Arc<PoolLoaders<P, N, LDT>>,
    pools: Vec<(EntityAddress, PoolClass)>,
    required_state: Option<RequiredState>,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    _n: PhantomData<N>,
}

impl<P, N, DB, LDT> RequiredPoolLoaderActor<P, N, DB, LDT>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Clone + Send + Sync + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    pub fn new(client: P, pool_loaders: Arc<PoolLoaders<P, N, LDT>>) -> Self {
        Self { client, pools: Vec::new(), pool_loaders, required_state: None, market: None, market_state: None, _n: PhantomData }
    }

    pub fn with_pool_address(self, address: Address, pool_class: PoolClass) -> Self {
        let mut pools = self.pools;
        pools.push((EntityAddress::Address(address), pool_class));
        Self { pools, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB, LDT>) -> Self {
        Self { market: Some(bc.market()), market_state: Some(state.market_state_commit()), ..self }
    }

    pub fn with_required_state(self, required_state: RequiredState) -> Self {
        Self { required_state: Some(required_state), ..self }
    }
}

impl<P, N, DB, LDT> Actor for RequiredPoolLoaderActor<P, N, DB, LDT>
where
    N: Network<TransactionRequest = LDT::TransactionRequest>,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database<Error = LoomDBError> + DatabaseRef<Error = LoomDBError> + DatabaseCommit + Send + Sync + Clone + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(required_pools_loader_worker(
            self.client.clone(),
            self.pool_loaders.clone(),
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
