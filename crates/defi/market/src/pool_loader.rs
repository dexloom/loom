use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::{eyre, Result};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tracing::{debug, error};

use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, SharedState, WorkerResult};
use loom_core_actors::{Accessor, Consumer};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::Blockchain;
use loom_defi_pools::protocols::{fetch_uni2_factory, fetch_uni3_factory, CurveProtocol};
use loom_defi_pools::{CurvePool, MaverickPool, PancakeV3Pool, UniswapV2Pool, UniswapV3Pool};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::required_state::RequiredStateReader;
use loom_types_entities::{get_protocol_by_factory, Market, MarketState, PoolClass, PoolProtocol, PoolWrapper};
use loom_types_events::Task;

use revm::{DatabaseCommit, DatabaseRef};

pub async fn pool_loader_worker<P, T, N, DB>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    tasks_rx: Broadcaster<Task>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    let mut fetch_tasks = FuturesUnordered::new();
    let mut processed_pools = HashMap::new();

    subscribe!(tasks_rx);
    loop {
        if let Ok(task) = tasks_rx.recv().await {
            let pools = match task {
                Task::FetchAndAddPools(pools) => pools,
                _ => continue,
            };

            for (pool_address, pool_class) in pools {
                // Check if pool already exists
                if processed_pools.insert(pool_address, true).is_some() {
                    continue;
                }
                // Fetch and add pool
                fetch_tasks.push(fetch_and_add_pool_by_address(
                    client.clone(),
                    market.clone(),
                    market_state.clone(),
                    pool_address,
                    pool_class,
                ));

                // Limit the number of concurrent fetch tasks
                if fetch_tasks.len() > 20 {
                    fetch_tasks.next().await;
                }
            }
        }
    }
}

/// Fetch pool data, add it to the market and fetch the required state
pub async fn fetch_and_add_pool_by_address<P, T, N, DB>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    pool_address: Address,
    pool_class: PoolClass,
) -> Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    debug!("Fetching pool {:#20x}", pool_address);

    match pool_class {
        PoolClass::UniswapV2 => {
            let factory_address = fetch_uni2_factory(client.clone(), pool_address).await?;
            let fetch_result = match get_protocol_by_factory(factory_address) {
                PoolProtocol::NomiswapStable | PoolProtocol::Miniswap | PoolProtocol::Integral | PoolProtocol::Safeswap => {
                    Err(eyre!("POOL_PROTOCOL_NOT_SUPPORTED"))
                }

                _ => {
                    let pool = UniswapV2Pool::fetch_pool_data(client.clone(), pool_address).await?;
                    fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), PoolWrapper::new(Arc::new(pool))).await
                }
            };

            if let Err(e) = fetch_result {
                error!("fetch_and_add_pool uni2 error {:#20x} : {}", pool_address, e)
            }
        }
        PoolClass::UniswapV3 => {
            let factory_address_result = fetch_uni3_factory(client.clone(), pool_address).await;
            match factory_address_result {
                Ok(factory_address) => {
                    let pool_wrapped = match get_protocol_by_factory(factory_address) {
                        PoolProtocol::PancakeV3 => {
                            PoolWrapper::new(Arc::new(PancakeV3Pool::fetch_pool_data(client.clone(), pool_address).await?))
                        }
                        PoolProtocol::Maverick => {
                            PoolWrapper::new(Arc::new(MaverickPool::fetch_pool_data(client.clone(), pool_address).await?))
                        }
                        _ => PoolWrapper::new(Arc::new(UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?)),
                    };

                    if let Err(e) = fetch_state_and_add_pool(client, market, market_state, pool_wrapped).await {
                        error!("fetch_and_add_pool uni3 error {:#20x} : {}", pool_address, e)
                    }
                }
                Err(e) => {
                    error!("Error fetching factory address at {:#20x}: {}", pool_address, e);
                    return Err(eyre!("CANNOT_GET_FACTORY_ADDRESS"));
                }
            }
        }
        PoolClass::Curve => match CurveProtocol::get_contract_from_code(client.clone(), pool_address).await {
            Ok(curve_contract) => {
                let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
                let pool_wrapped = PoolWrapper::new(Arc::new(curve_pool));

                match fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), pool_wrapped.clone()).await {
                    Err(e) => {
                        error!("Curve pool loading error {:?} : {}", pool_wrapped.get_address(), e);
                    }
                    Ok(_) => {
                        debug!("Curve pool loaded {:#20x}", pool_wrapped.get_address());
                    }
                }
            }
            Err(e) => {
                error!("Error getting curve contract from code {} : {} ", pool_address, e)
            }
        },
        _ => {
            error!("Error pool not supported at {:#20x}", pool_address);
            return Err(eyre!("POOL_CLASS_NOT_SUPPORTED"));
        }
    }
    Ok(())
}

pub async fn fetch_state_and_add_pool<P, T, N, DB>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    pool_wrapped: PoolWrapper,
) -> Result<()>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    match pool_wrapped.get_state_required() {
        Ok(required_state) => match RequiredStateReader::fetch_calls_and_slots(client, required_state, None).await {
            Ok(state) => {
                let pool_address = pool_wrapped.get_address();
                {
                    let mut market_state_write_guard = market_state.write().await;
                    market_state_write_guard.apply_geth_update(state);
                    market_state_write_guard.add_force_insert(pool_address);
                    market_state_write_guard.disable_cell_vec(pool_address, pool_wrapped.get_read_only_cell_vec());

                    drop(market_state_write_guard);
                }

                let directions_vec = pool_wrapped.get_swap_directions();
                let mut directions_tree: BTreeMap<PoolWrapper, Vec<(Address, Address)>> = BTreeMap::new();

                directions_tree.insert(pool_wrapped.clone(), directions_vec);

                let mut market_write_guard = market.write().await;
                // Ignore error if pool already exists because it was maybe already added by e.g. db pool loader
                let _ = market_write_guard.add_pool(pool_wrapped);

                let swap_paths = market_write_guard.build_swap_path_vec(&directions_tree)?;
                market_write_guard.add_paths(swap_paths);

                drop(market_write_guard)
            }
            Err(e) => {
                error!("{}", e);
                return Err(e);
            }
        },
        Err(e) => {
            error!("{}", e);
            return Err(e);
        }
    }

    Ok(())
}

#[derive(Accessor, Consumer)]
pub struct PoolLoaderActor<P, T, N, DB> {
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[consumer]
    tasks_rx: Option<Broadcaster<Task>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N, DB> PoolLoaderActor<P, T, N, DB>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone + Default + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, market: None, market_state: None, tasks_rx: None, _t: PhantomData, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain<DB>) -> Self {
        Self { market: Some(bc.market()), market_state: Some(bc.market_state()), tasks_rx: Some(bc.tasks_channel()), ..self }
    }
}

impl<P, T, N, DB> Actor for PoolLoaderActor<P, T, N, DB>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(pool_loader_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.tasks_rx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PoolLoaderActor"
    }
}
