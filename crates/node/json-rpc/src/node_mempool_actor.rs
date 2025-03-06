use alloy_network::{Ethereum, TransactionResponse};
use alloy_primitives::TxHash;
use alloy_provider::Provider;
use futures::StreamExt;
use tracing::error;

use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::*;
use loom_core_blockchain::Blockchain;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_blockchain::MempoolTx;
use loom_types_events::{MessageMempoolDataUpdate, NodeMempoolDataUpdate};

/// Worker listens for new transactions in the node mempool and broadcasts [`MessageMempoolDataUpdate`].
pub async fn new_node_mempool_worker<P>(client: P, name: String, mempool_tx: Broadcaster<MessageMempoolDataUpdate>) -> WorkerResult
where
    P: Provider<Ethereum> + Send + Sync + 'static,
{
    let mempool_subscription = client.subscribe_full_pending_transactions().await?;
    let mut stream = mempool_subscription.into_stream();

    while let Some(tx) = stream.next().await {
        let tx_hash: TxHash = tx.tx_hash();
        let update_msg: MessageMempoolDataUpdate = MessageMempoolDataUpdate::new_with_source(
            NodeMempoolDataUpdate { tx_hash, mempool_tx: MempoolTx { tx: Some(tx), ..MempoolTx::default() } },
            name.clone(),
        );
        if let Err(e) = mempool_tx.send(update_msg) {
            error!("mempool_tx.send error : {}", e);
            break;
        }
    }
    Ok(name)
}

#[derive(Producer)]
pub struct NodeMempoolActor<P> {
    name: &'static str,
    client: P,
    #[producer]
    mempool_tx: Option<Broadcaster<MessageMempoolDataUpdate>>,
}

impl<P> NodeMempoolActor<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> NodeMempoolActor<P> {
        NodeMempoolActor { client, name: "NodeMempoolActor", mempool_tx: None }
    }

    pub fn with_name(self, name: String) -> Self {
        Self { name: Box::leak(name.into_boxed_str()), ..self }
    }

    fn get_name(&self) -> &'static str {
        self.name
    }

    pub fn on_bc(self, bc: &Blockchain<LoomDataTypesEthereum>) -> Self {
        Self { mempool_tx: Some(bc.new_mempool_tx_channel()), ..self }
    }
}

impl<P> Actor for NodeMempoolActor<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
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
