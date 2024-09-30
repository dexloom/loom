use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState, PoolClass};
use defi_pools::{PoolsConfig};
use eyre::eyre;
use log::info;
use loom_actors::{Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::Accessor;
use std::marker::PhantomData;
use std::time::Instant;
//use pool_sync::{Chain, PoolInfo, PoolSync, PoolType};
use debug_provider::DebugProviderExt;

//pub const UNI_V2_FACTORY: Address = address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");

async fn pool_sync_loader_one_shot_worker<T, N, P>(
    _client: P,
    _market: SharedState<Market>,
    _market_state: SharedState<MarketState>,
    pools_config: PoolsConfig,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    if pools_config.is_enabled(PoolClass::UniswapV2) {
        let now = Instant::now();
        /*
        let pool_sync = PoolSync::builder()
            .add_pool(PoolType::UniswapV2)
            .chain(Chain::Ethereum)
            .build()?;
        let (pools, _last_synced_block) = pool_sync.sync_pools().await?;
        info!("UniswapV2 pools loaded in {:.2?} sec", now);

        let mut market_read_guard = market.write().await;
        let now = Instant::now();
        for pool in pools {
            market_read_guard.add_pool(UniswapV2Pool::new_with_data(
                pool.address(),
                pool.token0_address(),
                pool.token1_address(),
                UNI_V2_FACTORY,
                pool.get_v2().unwrap().token0_reserves.to(),
                pool.get_v2().unwrap().token1_reserves.to(),
            ))?;
        }
        drop(market_read_guard);
         */
        let elapsed = now.elapsed();
        info!("UniswapV2 pools added in {:.2?} sec", elapsed);
    }

    if pools_config.is_enabled(PoolClass::UniswapV3) {
        let now = Instant::now();
        /*
        let pool_sync = PoolSync::builder()
            .add_pool(PoolType::UniswapV3)
            .chain(Chain::Ethereum)
            .build()?;
        let (pools, _last_synced_block) = pool_sync.sync_pools().await?;
        let elapsed = now.elapsed();
        info!("UniswapV3 pools loaded {} in {:.2?} sec", pools.len(), elapsed);

        let mut market_state_read_guard = market.write().await;
        for pool in pools {
            // TODO: Load real pool
            market_state_read_guard.add_empty_pool(&pool.address())?;
        }
        drop(market_state_read_guard);
         */
        let elapsed = now.elapsed();
        info!("UniswapV3 pools added in {:.2?} sec", elapsed);
    }

    Ok("PoolSyncLoaderOneShotActor finished".to_string())
}

/// The one-shot actor loads all existing Uniswap v2 pairs and v3 pools.
#[derive(Accessor)]
pub struct PoolSyncLoaderOneShotActor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    client: P,
    market: Option<SharedState<Market>>,
    market_state: Option<SharedState<MarketState>>,
    pools_config: PoolsConfig,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<T, N, P> PoolSyncLoaderOneShotActor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, pools_config: PoolsConfig) -> PoolSyncLoaderOneShotActor<T, N, P> {
        PoolSyncLoaderOneShotActor { client, market: None, market_state: None, pools_config, _t: PhantomData, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market().clone()), market_state: Some(bc.market_state().clone()), ..self }
    }
}

impl<T, N, P> Actor for PoolSyncLoaderOneShotActor<T, N, P>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start_and_wait(&self) -> eyre::Result<()> {
        let rt = tokio::runtime::Runtime::new()?; // we need a different runtime to wait for the result
        let client = self.client.clone();
        let market = self.market.clone().unwrap();
        let market_state = self.market_state.clone().unwrap();
        let pools_config = self.pools_config.clone();
        let handle = rt.spawn(async { pool_sync_loader_one_shot_worker(client, market, market_state, pools_config).await });

        self.wait(Ok(vec![handle]))?;
        rt.shutdown_background();

        Ok(())
    }
    fn start(&self) -> ActorResult {
        Err(eyre!("NEED_TO_BE_WAITED"))
    }

    fn name(&self) -> &'static str {
        "PoolSyncLoaderOneShotActor"
    }
}
