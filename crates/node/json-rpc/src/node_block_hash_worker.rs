use std::collections::HashMap;

use alloy_network::Ethereum;
use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_pubsub::PubSubConnect;
use alloy_rpc_types::Header;
use alloy_transport::Transport;
use chrono::Utc;
use eyre::Result;
use futures::StreamExt;
use loom_core_actors::{run_async, Broadcaster, WorkerResult};
use loom_types_events::{BlockHeader, MessageBlockHeader};
use tracing::{error, info};

#[allow(dead_code)]
pub async fn new_node_block_hash_worker<P: Provider + PubSubConnect>(client: P, sender: Broadcaster<Header>) -> Result<()> {
    info!("Starting node block hash worker");
    let sub = client.subscribe_blocks().await?;

    let mut block_processed: HashMap<BlockHash, chrono::DateTime<Utc>> = HashMap::new();

    let mut stream = sub.into_stream();

    loop {
        tokio::select! {
            header = stream.next() => {
                if let Some(header) = header {
                    info!("Block hash received: {:?}" , header);
                    if let std::collections::hash_map::Entry::Vacant(e) = block_processed.entry(header.hash) {
                        e.insert(Utc::now());
                        run_async!(sender.send(header));
                        block_processed.retain(|_, &mut v| v > Utc::now() - chrono::TimeDelta::minutes(10) );
                    }
                }

            }
        }
    }
}

pub async fn new_node_block_header_worker<P, T>(
    client: P,
    new_block_header_channel: Broadcaster<Header>,
    block_header_channel: Broadcaster<MessageBlockHeader>,
) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    info!("Starting node block header worker");
    let sub = client.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    let mut block_processed: HashMap<BlockHash, chrono::DateTime<Utc>> = HashMap::new();

    loop {
        tokio::select! {
            block_msg = stream.next() => {
                if let Some(block_header) = block_msg {
                    let block_hash = block_header.hash;
                    info!("Block hash received: {:?}" , block_hash);
                    if let std::collections::hash_map::Entry::Vacant(e) = block_processed.entry(block_hash) {
                        e.insert(Utc::now());
                        if let Err(e) =  new_block_header_channel.send(block_header.clone()).await {
                            error!("Block hash broadcaster error  {}", e);
                        }
                        if let Err(e) = block_header_channel.send(MessageBlockHeader::new_with_time(BlockHeader::new(block_header))).await {
                            error!("Block header broadcaster error {}", e);
                        }
                    }
                }
            }
        }
    }
}
