use loom_node_grpc_exex_proto::proto::{remote_ex_ex_client::RemoteExExClient, SubscribeRequest};
use reth_exex::ExExNotification;
use reth_tracing::tracing::error;
use reth_tracing::{tracing::info, RethTracer, Tracer};
use tokio::select;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _ = RethTracer::new().init()?;

    let mut client =
        RemoteExExClient::connect("http://[::1]:10000").await?.max_encoding_message_size(usize::MAX).max_decoding_message_size(usize::MAX);

    let mut stream_exex = client.subscribe_ex_ex(SubscribeRequest {}).await?.into_inner();
    let mut stream_tx = client.subscribe_mempool_tx(SubscribeRequest {}).await?.into_inner();

    loop {
        select! {
            notification = stream_exex.message() => {
                match notification {
                    Ok(notification) => {
                        if let Some(notification) = notification {
                            let notification = ExExNotification::try_from(&notification)?;

                            match notification {
                                ExExNotification::ChainCommitted { new } => {
                                    info!(committed_chain = ?new.range(), "Received commit");
                                }
                                ExExNotification::ChainReorged { old, new } => {
                                    info!(from_chain = ?old.range(), to_chain = ?new.range(), "Received reorg");
                                }
                                ExExNotification::ChainReverted { old } => {
                                    info!(reverted_chain = ?old.range(), "Received revert");
                                }
                            };
                        }

                    },
                    Err(e)=>{
                        error!(error=?e, "stream_exex.message");
                    }
                }
            },
            notification = stream_tx.message() =>{
                match notification {
                    Ok(notification) => {
                        if let Some(tx) = notification {
                            info!(hash=?tx.hash, "tx received")
                        }
                    }
                    Err(e)=>{
                        error!(error=?e, "stream_tx.message");
                    }
                }
            },
        }
    }
}
