use std::marker::PhantomData;

use alloy_network::Ethereum;
use alloy_primitives::TxHash;
use alloy_provider::Provider;
use alloy_transport::Transport;
use futures::StreamExt;
use log::error;

use defi_blockchain::Blockchain;
use defi_events::{MessageMempoolDataUpdate, NodeMempoolDataUpdate};
use defi_types::MempoolTx;
use loom_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_actors_macros::*;

pub async fn new_node_mempool_worker<P, T>(client: P, name: String, mempool_tx: Broadcaster<MessageMempoolDataUpdate>) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + 'static,
{
    let mempool_subscription = client.subscribe_full_pending_transactions().await?;
    let mut stream = mempool_subscription.into_stream();

    while let Some(tx) = stream.next().await {
        let tx_hash: TxHash = tx.hash;
        let update_msg: MessageMempoolDataUpdate = MessageMempoolDataUpdate::new_with_source(
            NodeMempoolDataUpdate { tx_hash, mempool_tx: MempoolTx { tx: Some(tx), ..MempoolTx::default() } },
            name.clone(),
        );
        if let Err(e) = mempool_tx.send(update_msg).await {
            error!("mempool_tx.send error : {}", e);
            break;
        }
    }
    Ok(name)
}

#[derive(Producer)]
pub struct NodeMempoolActor<P, T> {
    name: &'static str,
    client: P,
    #[producer]
    mempool_tx: Option<Broadcaster<MessageMempoolDataUpdate>>,
    _t: PhantomData<T>,
}

impl<P, T> NodeMempoolActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> NodeMempoolActor<P, T> {
        NodeMempoolActor { client, name: "NodeMempoolActor", mempool_tx: None, _t: PhantomData }
    }

    pub fn with_name(self, name: String) -> Self {
        Self { name: Box::leak(name.into_boxed_str()), ..self }
    }

    fn get_name(&self) -> &'static str {
        self.name
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { mempool_tx: Some(bc.new_mempool_tx_channel()), ..self }
    }
}

impl<P, T> Actor for NodeMempoolActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task =
            tokio::task::spawn(new_node_mempool_worker(self.client.clone(), self.name.to_string(), self.mempool_tx.clone().unwrap()));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        self.get_name()
    }
}
