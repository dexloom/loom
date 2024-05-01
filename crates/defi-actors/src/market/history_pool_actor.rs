use std::convert::Infallible;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use async_trait::async_trait;
use eyre::Result;
use log::{error, info, Log};

use debug_provider::DebugProviderExt;
use defi_entities::{Market, MarketState};
use loom_actors::{Accessor, Actor, ActorResult, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

use crate::market::logs_parser::process_log_entries;

async fn history_pool_loader_worker<P>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> WorkerResult
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    let mut current_block = client.get_block_number().await.unwrap();

    //let mut current_block = U64::from(17836224);
    let block_size: u64 = 5;

    for i in 1..1000 {
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


#[derive(Accessor, Consumer)]
pub struct HistoryPoolLoaderActor<P>
{
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
}

impl<P> HistoryPoolLoaderActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            market: None,
            market_state: None,
        }
    }
}

#[async_trait]
impl<P> Actor for HistoryPoolLoaderActor<P>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
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
