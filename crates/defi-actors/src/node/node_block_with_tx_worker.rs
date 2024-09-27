use alloy_network::{Ethereum, HeaderResponse};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockTransactionsKind, Header};
use alloy_transport::Transport;
use defi_events::{Message, MessageBlock};
use log::error;
use loom_actors::{subscribe, Broadcaster, WorkerResult};

pub async fn new_block_with_tx_worker<P, T>(
    client: P,
    block_header_receiver: Broadcaster<Header>,
    sender: Broadcaster<MessageBlock>,
) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + 'static,
{
    subscribe!(block_header_receiver);

    loop {
        if let Ok(block_header) = block_header_receiver.recv().await {
            if let Ok(Some(block_with_txes)) = client.get_block_by_hash(block_header.hash(), BlockTransactionsKind::Full).await {
                if let Err(e) = sender.send(Message::new_with_time(block_with_txes)).await {
                    error!("Broadcaster error {}", e);
                }
            }
        }
    }
}
