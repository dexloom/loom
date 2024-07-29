use defi_events::{MessageTxCompose, RlpState, TxCompose};
use defi_types::Mempool;
use log::{error, info};
use loom_actors::{Broadcaster, SharedState, WorkerResult};
use loom_utils::reth_types::decode_into_transaction;
use tokio::select;

pub(crate) async fn replayer_compose_worker(mempool: SharedState<Mempool>, compose_channel: Broadcaster<MessageTxCompose>) -> WorkerResult {
    let mut compose_channel_rx = compose_channel.subscribe().await;

    loop {
        select! {
            msg = compose_channel_rx.recv() => {
                if let Ok(msg) = msg {
                    if let TxCompose::Broadcast(broadcast_msg) = msg.inner {
                        info!("Broadcast compose message received. {:?}", broadcast_msg.tx_bundle);
                        for tx in broadcast_msg.rlp_bundle.unwrap_or_default() {
                            match tx {
                                RlpState::Backrun( rlp_tx) | RlpState::Stuffing( rlp_tx)=>{
                                    match decode_into_transaction( &rlp_tx ) {
                                        Ok(new_tx)=>{
                                            mempool.write().await.add_tx(new_tx);
                                        }
                                        Err(e)=>{
                                            error!("decode_into_transaction {}", e);
                                        }
                                    }

                                }
                                _=>{
                                    error!("Unknown RLP tx type");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
