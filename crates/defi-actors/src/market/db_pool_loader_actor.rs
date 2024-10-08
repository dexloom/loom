use alloy_eips::BlockNumberOrTag;
use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState, PoolClass, RethAdapter};
use defi_pools::{PoolsConfig, Slot0, UniswapV2Pool, UniswapV3Pool};
use loom_actors::{Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::Accessor;
use reth_node_api::{FullNodeComponents, NodeAddOns};
use reth_provider::StateProviderFactory;
use rethdb_dexsync::univ2::{UniV2Factory, UNI_V2_FACTORY};
use rethdb_dexsync::univ3::{UniV3PositionManager, UNI_V3_FACTORY, UNI_V3_POSITION_MANAGER};
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::{error, info};

async fn pool_loader_one_shot_worker<Node, AddOns>(
    reth_adapter: RethAdapter<Node, AddOns>,
    market: SharedState<Market>,
    _market_state: SharedState<MarketState>,
    pools_config: PoolsConfig,
) -> WorkerResult
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    if pools_config.is_enabled(PoolClass::UniswapV2) {
        let now = Instant::now();

        let (tx, rx) = oneshot::channel();
        let node = reth_adapter.node.clone().unwrap();
        node.task_executor.spawn_blocking(Box::pin(async move {
            let univ2_factory = match UniV2Factory::load_pairs(&&node.provider, &BlockNumberOrTag::Latest, UNI_V2_FACTORY, None) {
                Ok(univ2_factory) => univ2_factory,
                Err(e) => {
                    error!("Failed to load UniswapV2 pairs: {:?}", e);
                    tx.send(Err(e)).unwrap();
                    return;
                }
            };
            tx.send(Ok(univ2_factory)).unwrap();
        }));

        let univ2_factory = rx.await??;
        let elapsed = now.elapsed();
        info!("UniswapV2 {} pools loaded in {:.2?} sec", univ2_factory.pairs.len(), elapsed);
        let mut market_write_guard = market.write().await;

        let now = Instant::now();
        for (pair, reserve) in univ2_factory.pairs {
            // ignore error if pool already exists
            let _ = market_write_guard.add_pool(UniswapV2Pool::new_with_data(
                pair.address,
                pair.token0,
                pair.token1,
                UNI_V2_FACTORY,
                reserve.reserve0.to(),
                reserve.reserve1.to(),
            ));
        }
        drop(market_write_guard);
        let elapsed = now.elapsed();
        info!("UniswapV2 pools added in {:.2?} sec", elapsed);
    }

    if pools_config.is_enabled(PoolClass::UniswapV3) {
        let now = Instant::now();

        let (tx, rx) = oneshot::channel();
        let node = reth_adapter.node.clone().unwrap();
        node.task_executor.spawn_blocking(Box::pin(async move {
            let position_manager = match UniV3PositionManager::load_pools(node.provider.latest().unwrap(), UNI_V3_POSITION_MANAGER) {
                Ok(position_manager) => position_manager,
                Err(e) => {
                    error!("Failed to load UniswapV3 pools: {:?}", e);
                    tx.send(Err(e)).unwrap();
                    return;
                }
            };
            tx.send(Ok(position_manager)).unwrap();
        }));
        let position_manager = rx.await??;
        let elapsed = now.elapsed();
        info!("UniswapV3 {} pools loaded in {:.2?} sec", position_manager.pools.len(), elapsed);

        let now = Instant::now();
        let mut market_write_guard = market.write().await;
        for (pool, slot0, liquidity) in position_manager.pools {
            let slot0 = Slot0 {
                sqrt_price_x96: slot0.sqrt_price_x96.to(),
                tick: slot0.tick.as_i32(),
                observation_index: slot0.observation_index.to(),
                observation_cardinality: slot0.observation_cardinality.to(),
                observation_cardinality_next: slot0.observation_cardinality_next.to(),
                fee_protocol: slot0.fee_protocol,
                unlocked: slot0.unlocked,
            };
            // ignore error if pool already exists
            let _ = market_write_guard.add_pool(UniswapV3Pool::new_with_data(
                pool.address,
                pool.token0,
                pool.token1,
                liquidity.to::<u128>(),
                pool.fee.to::<u32>(),
                Some(slot0),
                UNI_V3_FACTORY,
            ));
        }
        drop(market_write_guard);
        let elapsed = now.elapsed();
        info!("UniswapV3 pools added in {:.2?} sec", elapsed);
    }

    Ok("DbPoolLoaderOneShotActor finished".to_string())
}

/// The one-shot actor reads all existing uniswap v2 pairs and v3 pools.
#[derive(Accessor)]
pub struct DbPoolLoaderOneShotActor<Node, AddOns>
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    reth_adapter: RethAdapter<Node, AddOns>,
    market: Option<SharedState<Market>>,
    market_state: Option<SharedState<MarketState>>,
    pools_config: PoolsConfig,
}

impl<Node, AddOns> DbPoolLoaderOneShotActor<Node, AddOns>
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    pub fn new(reth_adapter: RethAdapter<Node, AddOns>, pools_config: PoolsConfig) -> DbPoolLoaderOneShotActor<Node, AddOns> {
        DbPoolLoaderOneShotActor { reth_adapter, market: None, market_state: None, pools_config }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market().clone()), market_state: Some(bc.market_state().clone()), ..self }
    }
}

impl<Node, AddOns> Actor for DbPoolLoaderOneShotActor<Node, AddOns>
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(pool_loader_one_shot_worker(
            self.reth_adapter.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.pools_config.clone(),
        ));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "DbPoolLoaderOneShotActor"
    }
}
