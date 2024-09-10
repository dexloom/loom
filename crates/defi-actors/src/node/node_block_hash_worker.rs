use std::collections::HashMap;

use alloy_network::Ethereum;
use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_pubsub::PubSubConnect;
use alloy_rpc_types::{Block, Header};
use alloy_transport::Transport;
use chrono::Utc;
use eyre::Result;
use futures::StreamExt;
use log::{error, info};

use loom_actors::{run_async, Broadcaster, WorkerResult};

#[allow(dead_code)]
pub async fn new_node_block_hash_worker<P: Provider + PubSubConnect>(client: P, sender: Broadcaster<BlockHash>) -> Result<()> {
    info!("Starting node block hash worker");
    let sub = client.subscribe_blocks().await?;

    let mut block_processed: HashMap<BlockHash, chrono::DateTime<Utc>> = HashMap::new();

    let mut stream = sub.into_stream();

    loop {
        tokio::select! {
            block = stream.next() => {
                if let Some(block) = block {
                    info!("Block hash received: {:?}" , block.header.hash );
                    if let std::collections::hash_map::Entry::Vacant(e) = block_processed.entry(block.header.hash) {
                        e.insert(Utc::now());
                        run_async!(sender.send(block.header.hash))
                    }
                }

            }
        }
    }
}

pub async fn new_node_block_header_worker<P, T>(
    client: P,
    block_hash_channel: Broadcaster<BlockHash>,
    block_header_channel: Broadcaster<Header>,
) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    info!("Starting node block hash worker");
    let sub = client.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    let mut block_processed: HashMap<BlockHash, chrono::DateTime<Utc>> = HashMap::new();

    loop {
        tokio::select! {
            block_msg = stream.next() => {
                if let Some(block_header) = block_msg {
                    let block : Block = block_header;
                    let block_hash = block.header.hash;
                    info!("Block hash received: {:?}" , block_hash);
                    if let std::collections::hash_map::Entry::Vacant(e) = block_processed.entry(block_hash) {
                        e.insert(Utc::now());
                        if let Err(e) =  block_hash_channel.send(block_hash).await {
                            error!("Block hash broadcaster error  {}", e);
                        }
                        if let Err(e) = block_header_channel.send(block.header).await {
                            error!("Block header broadcaster error {}", e);
                        }
                    }
                }
            }
        }
    }
}
