use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_transport::Transport;
use async_trait::async_trait;
use log::{error, info};

use debug_provider::DebugProviderExt;
use defi_entities::{Market, MarketState};
use loom_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::Accessor;

use crate::market::logs_parser::process_log_entries;

async fn history_pool_loader_worker<P, T, N>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> WorkerResult
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let mut current_block = client.get_block_number().await.unwrap();

    //let mut current_block = U64::from(17836224);
    let block_size: u64 = 5;

    for _ in 1..10000 {
        current_block -= block_size;
        info!("Loading blocks {} {}", current_block, current_block + block_size);
        let filter = Filter::new().from_block(current_block).to_block(current_block + block_size - 1);
        match client.get_logs(&filter).await {
            Ok(logs) => {
                let _ = process_log_entries(
                    client.clone(),
                    market.clone(),
                    market_state.clone(),
                    logs,
                ).await;
            }
            Err(e) => { error!("{}", e) }
        }
    }
    info!("history_pool_loader_worker finished");


    Ok("history_pool_loader_worker".to_string())
}


#[derive(Accessor)]
pub struct HistoryPoolLoaderActor<P, T, N>
{
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> HistoryPoolLoaderActor<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            market: None,
            market_state: None,
            _t: PhantomData::default(),
            _n: PhantomData::default(),
        }
    }
}

#[async_trait]
impl<P, T, N> Actor for HistoryPoolLoaderActor<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            history_pool_loader_worker(
                self.client.clone(),
                self.market.clone().unwrap(),
                self.market_state.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }
}
