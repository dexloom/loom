use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_transport::Transport;
use log::{debug, error, info};

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState};
use defi_pools::PoolsConfig;
use loom_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::Accessor;

use crate::market::logs_parser::process_log_entries;

async fn history_pool_loader_worker<P, T, N>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    pools_config: PoolsConfig,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let mut current_block = client.get_block_number().await.unwrap();

    let block_size: u64 = 5;

    for _ in 1..10000 {
        if current_block < block_size + 1 {
            break;
        }
        current_block -= block_size;
        debug!("Loading blocks {} {}", current_block, current_block + block_size);
        let filter = Filter::new().from_block(current_block).to_block(current_block + block_size - 1);
        match client.get_logs(&filter).await {
            Ok(logs) => {
                let _ = process_log_entries(client.clone(), market.clone(), market_state.clone(), logs, &pools_config).await;
            }
            Err(e) => {
                error!("{}", e)
            }
        }
    }
    info!("history_pool_loader_worker finished");

    Ok("history_pool_loader_worker".to_string())
}

#[derive(Accessor)]
pub struct HistoryPoolLoaderActor<P, T, N> {
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    pools_config: PoolsConfig,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> HistoryPoolLoaderActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, pools_config: PoolsConfig) -> Self {
        Self { client, market: None, market_state: None, pools_config, _t: PhantomData, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), market_state: Some(bc.market_state()), ..self }
    }
}

impl<P, T, N> Actor for HistoryPoolLoaderActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(history_pool_loader_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.pools_config.clone(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "HistoryPoolLoaderActor"
    }
}
