use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState, PoolClass, RethAdapter};
use defi_pools::{PoolsConfig, UniswapV2Pool};
use eyre::eyre;
use log::info;
use loom_actors::{Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::Accessor;
use reth_direct_db_uniswap_storage::{UniV2Factory, UniV3PositionManager, UNI_V2_FACTORY, UNI_V3_POSITION_MANAGER};
use reth_node_api::{FullNodeComponents, NodeAddOns};
use std::time::Instant;

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
        let univ2_factory = UniV2Factory::load_pairs(reth_adapter.latest()?, UNI_V2_FACTORY)?;
        let elapsed = now.elapsed();
        info!("UniswapV2 {} pools loaded in {:.2?} sec", univ2_factory.pairs.len(), elapsed);
        let mut market_read_guard = market.write().await;

        let now = Instant::now();
        for (pair, reserve) in univ2_factory.pairs {
            market_read_guard.add_pool(UniswapV2Pool::new_with_data(
                pair.address,
                pair.token0,
                pair.token1,
                UNI_V2_FACTORY,
                reserve.reserve0.to(),
                reserve.reserve1.to(),
            ))?;
        }
        drop(market_read_guard);
        let elapsed = now.elapsed();
        info!("UniswapV2 pools added in {:.2?} sec", elapsed);
    }

    if pools_config.is_enabled(PoolClass::UniswapV3) {
        let now = Instant::now();
        let position_manager = UniV3PositionManager::load_pools(reth_adapter.latest()?, UNI_V3_POSITION_MANAGER)?;
        let elapsed = now.elapsed();
        info!("UniswapV3 {} pools loaded in {:.2?} sec", position_manager.pools.len(), elapsed);

        let now = Instant::now();
        let mut market_state_read_guard = market.write().await;
        for pool in position_manager.pools {
            // TODO: Load pool ticks, etc
            market_state_read_guard.add_empty_pool(&pool.address)?;
        }
        drop(market_state_read_guard);
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
    fn start_and_wait(&self) -> eyre::Result<()> {
        let rt = tokio::runtime::Runtime::new()?; // we need a different runtime to wait for the result
        let reth_adapter = self.reth_adapter.clone();
        let market = self.market.clone().unwrap();
        let market_state = self.market_state.clone().unwrap();
        let pools_config = self.pools_config.clone();
        let handle = rt.spawn(async { pool_loader_one_shot_worker(reth_adapter, market, market_state, pools_config).await });

        self.wait(Ok(vec![handle]))?;
        rt.shutdown_background();

        Ok(())
    }
    fn start(&self) -> ActorResult {
        Err(eyre!("NEED_TO_BE_WAITED"))
    }

    fn name(&self) -> &'static str {
        "DbPoolLoaderOneShotActor"
    }
}
