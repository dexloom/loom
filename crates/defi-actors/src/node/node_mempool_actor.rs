use alloy_primitives::TxHash;
use alloy_provider::Provider;
use async_trait::async_trait;
use futures::StreamExt;
use log::error;

use defi_events::{MempoolDataUpdate, MessageMempoolDataUpdate};
use defi_types::MempoolTx;
use loom_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_actors_macros::*;

pub async fn new_node_mempool_worker<P>(
    client: P,
    name: String,
    mempool_tx: Broadcaster<MessageMempoolDataUpdate>,
) -> WorkerResult
    where
        P: Provider + Send + Sync + 'static,
{
    let mempool_subscription = client.subscribe_full_pending_transactions().await?;
    let mut stream = mempool_subscription.into_stream();

    while let Some(tx) = stream.next().await {
        let tx_hash: TxHash = tx.hash;
        let update_msg: MessageMempoolDataUpdate = MessageMempoolDataUpdate::new_with_source(MempoolDataUpdate { tx_hash, mempool_tx: MempoolTx { tx: Some(tx), ..MempoolTx::default() } }, name.clone());
        match mempool_tx.send(update_msg).await {
            Err(e) => {
                error!("{}", e);
            }
            _ => {}
        }
    }
    Ok(name)
}

#[derive(Producer)]
pub struct NodeMempoolActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    client: P,
    name: String,
    #[producer]
    mempool_tx: Option<Broadcaster<MessageMempoolDataUpdate>>,
}

impl<P> NodeMempoolActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, name: String) -> NodeMempoolActor<P> {
        NodeMempoolActor {
            client,
            name,
            mempool_tx: None,
        }
    }
}


#[async_trait]
impl<P> Actor for NodeMempoolActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            new_node_mempool_worker(
                self.client.clone(),
                self.name.clone(),
                self.mempool_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }
}

