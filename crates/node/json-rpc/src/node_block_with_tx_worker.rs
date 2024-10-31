use alloy_network::{Ethereum, HeaderResponse};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockTransactionsKind, Header};
use alloy_transport::Transport;
use loom_core_actors::{subscribe, Broadcaster, WorkerResult};
use loom_types_events::{Message, MessageBlock};
use tracing::{debug, error};

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
            let (block_number, block_hash) = (block_header.number, block_header.hash);
            debug!("BlockWithTx header received {} {}", block_number, block_hash);

            let mut err_counter = 0;

            while err_counter < 3 {
                match client.get_block_by_hash(block_header.hash(), BlockTransactionsKind::Full).await {
                    Ok(block_with_tx) => {
                        if let Some(block_with_txes) = block_with_tx {
                            if let Err(e) = sender.send(Message::new_with_time(block_with_txes)).await {
                                error!("Broadcaster error {}", e);
                            }
                        } else {
                            error!("BlockWithTx is empty");
                        }
                        break;
                    }
                    Err(e) => {
                        error!("client.get_block_by_hash {e}");
                        err_counter += 1;
                    }
                }
            }

            debug!("BlockWithTx processing finished {} {}", block_number, block_hash);
        }
    }
}
