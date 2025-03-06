use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Network;
use alloy_provider::Provider;
use eyre::Result;
use tracing::{debug, error, info};

use loom_core_actors::{run_sync, subscribe, Actor, ActorResult, Broadcaster, Producer, SharedState, WorkerResult};
use loom_core_actors::{Accessor, Consumer};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::required_state::RequiredStateReader;
use loom_types_entities::{Market, MarketState, PoolClass, PoolId, PoolLoaders, PoolWrapper, SwapDirection};
use loom_types_events::{LoomTask, MarketEvents};

use loom_types_blockchain::get_touched_addresses;
use loom_types_entities::pool_config::PoolsLoadingConfig;
use revm::{Database, DatabaseCommit, DatabaseRef};
use tokio::sync::Semaphore;

const MAX_CONCURRENT_TASKS: usize = 20;

pub async fn pool_loader_worker<P, PL, N, DB>(
    client: P,
    pool_loaders: Arc<PoolLoaders<PL, N>>,
    pools_config: PoolsLoadingConfig,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    tasks_rx: Broadcaster<LoomTask>,
    market_events_tx: Broadcaster<MarketEvents>,
) -> WorkerResult
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    let mut processed_pools = HashMap::new();
    let semaphore = std::sync::Arc::new(Semaphore::new(pools_config.threads().unwrap_or(MAX_CONCURRENT_TASKS)));

    subscribe!(tasks_rx);
    loop {
        if let Ok(task) = tasks_rx.recv().await {
            let pools = match task {
                LoomTask::FetchAndAddPools(pools) => pools,
            };

            for (pool_id, pool_class) in pools {
                // Check if pool already exists
                if processed_pools.insert(pool_id, true).is_some() {
                    continue;
                }

                let sema_clone = semaphore.clone();
                let client_clone = client.clone();
                let market_clone = market.clone();
                let market_state = market_state.clone();
                let pool_loaders_clone = pool_loaders.clone();
                let market_events_tx_clone = market_events_tx.clone();

                tokio::task::spawn(async move {
                    match sema_clone.acquire().await {
                        Ok(permit) => {
                            match fetch_and_add_pool_by_pool_id(
                                client_clone,
                                market_clone,
                                market_state,
                                pool_loaders_clone,
                                pool_id,
                                pool_class,
                            )
                            .await
                            {
                                Ok((pool_id, swap_path_idx_vec)) => {
                                    info!(%pool_id, %pool_class, "Pool loaded successfully");
                                    run_sync!(market_events_tx_clone.send(MarketEvents::NewPoolLoaded { pool_id, swap_path_idx_vec }))
                                }
                                Err(error) => {
                                    error!(%error, %pool_id, %pool_class, "failed fetch_and_add_pool_by_address");
                                }
                            }

                            drop(permit);
                        }
                        Err(error) => {
                            error!(%error, "failed acquire semaphore");
                        }
                    }
                });
            }
        }
    }
}

/// Fetch pool data, add it to the market and fetch the required state
pub async fn fetch_and_add_pool_by_pool_id<P, PL, N, DB>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    pool_loaders: Arc<PoolLoaders<PL, N>>,
    pool_id: PoolId,
    pool_class: PoolClass,
) -> Result<(PoolId, Vec<usize>)>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Database + DatabaseCommit + Send + Sync + Clone + 'static,
{
    debug!(%pool_id, %pool_class, "Fetching pool");

    let pool = pool_loaders.load_pool_without_provider(pool_id, &pool_class).await?;
    fetch_state_and_add_pool(client, market.clone(), market_state.clone(), pool).await
}

pub async fn fetch_state_and_add_pool<P, N, DB>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    pool_wrapped: PoolWrapper,
) -> Result<(PoolId, Vec<usize>)>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    match pool_wrapped.get_state_required() {
        Ok(required_state) => match RequiredStateReader::fetch_calls_and_slots(client, required_state, None).await {
            Ok(state) => {
                let pool_address = pool_wrapped.get_address();
                {
                    let updated_addresses = get_touched_addresses(&state);

                    let mut market_state_write_guard = market_state.write().await;
                    market_state_write_guard.apply_geth_update(state);
                    market_state_write_guard.config.disable_cell_vec(pool_address, pool_wrapped.get_read_only_cell_vec());

                    let pool_tokens = pool_wrapped.get_tokens();

                    for updated_address in updated_addresses {
                        if !pool_tokens.contains(&updated_address) {
                            market_state_write_guard.config.add_force_insert(updated_address);
                        }
                    }

                    drop(market_state_write_guard);
                }

                let directions_vec = pool_wrapped.get_swap_directions();
                let pool_manager_cells = pool_wrapped.get_pool_manager_cells();
                let pool_id = pool_wrapped.get_pool_id();

                let mut directions_tree: BTreeMap<PoolWrapper, Vec<SwapDirection>> = BTreeMap::new();
                directions_tree.insert(pool_wrapped.clone(), directions_vec);

                let start_time = std::time::Instant::now();
                let mut market_write_guard = market.write().await;
                debug!(elapsed = start_time.elapsed().as_micros(), "market_guard market.write acquired");
                // Ignore error if pool already exists because it was maybe already added by e.g. db pool loader
                let _ = market_write_guard.add_pool(pool_wrapped);

                let swap_paths = market_write_guard.build_swap_path_vec(&directions_tree)?;
                let swap_paths_added = market_write_guard.add_paths(swap_paths);

                for (pool_manager_address, cells_vec) in pool_manager_cells {
                    for cell in cells_vec {
                        market_write_guard.add_pool_manager_cell(pool_manager_address, pool_id, cell)
                    }
                }

                debug!(elapsed = start_time.elapsed().as_micros(),  market = %market_write_guard, "market_guard path added");

                drop(market_write_guard);
                debug!(elapsed = start_time.elapsed().as_micros(), "market_guard market.write releases");

                Ok((pool_id, swap_paths_added))
            }
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        },
        Err(e) => {
            error!("{}", e);
            Err(e)
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct PoolLoaderActor<P, PL, N, DB>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + Default + 'static,
{
    client: P,
    pool_loaders: Arc<PoolLoaders<PL, N>>,
    pools_config: PoolsLoadingConfig,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[consumer]
    tasks_rx: Option<Broadcaster<LoomTask>>,
    #[producer]
    market_events_channel_tx: Option<Broadcaster<MarketEvents>>,
    _n: PhantomData<N>,
}

impl<P, PL, N, DB> PoolLoaderActor<P, PL, N, DB>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + Default + 'static,
{
    pub fn new(client: P, pool_loaders: Arc<PoolLoaders<PL, N>>, pools_config: PoolsLoadingConfig) -> Self {
        Self {
            client,
            pool_loaders,
            pools_config,
            market: None,
            market_state: None,
            tasks_rx: None,
            market_events_channel_tx: None,
            _n: PhantomData,
        }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>) -> Self {
        Self {
            market: Some(bc.market()),
            market_state: Some(state.market_state_commit()),
            tasks_rx: Some(bc.tasks_channel()),
            market_events_channel_tx: Some(bc.market_events_channel()),
            ..self
        }
    }
}

impl<P, PL, N, DB> Actor for PoolLoaderActor<P, PL, N, DB>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Default + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(pool_loader_worker(
            self.client.clone(),
            self.pool_loaders.clone(),
            self.pools_config.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.tasks_rx.clone().unwrap(),
            self.market_events_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PoolLoaderActor"
    }
}
