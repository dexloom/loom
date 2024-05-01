use alloy_provider::Provider;
use alloy_rpc_types::{Block, Header};
use async_trait::async_trait;
use tokio::task::JoinHandle;

use defi_events::{BlockLogsUpdate, BlockStateUpdate};
use loom_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_actors_macros::Producer;

use crate::node::node_block_hash_worker::new_node_block_header_worker;
use crate::node::node_block_logs_worker::new_node_block_logs_worker;
use crate::node::node_block_state_worker::new_node_block_state_worker;
use crate::node::node_block_with_tx_worker::new_block_with_tx_worker;

pub async fn new_node_block_starer<P>(client: P,
                                      new_block_headers_channel: Option<Broadcaster<Header>>,
                                      new_block_with_tx_channel: Option<Broadcaster<Block>>,
                                      new_block_logs_channel: Option<Broadcaster<BlockLogsUpdate>>,
                                      new_block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
) -> ActorResult
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    let new_block_hash_channel = Broadcaster::new(10);
    let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

    match new_block_with_tx_channel {
        Some(channel) => {
            tasks.push(tokio::task::spawn(
                new_block_with_tx_worker(client.clone(), new_block_hash_channel.clone().subscribe().await, channel)
            ));
        }
        None => {}
    }

    match new_block_headers_channel {
        Some(channel) => {
            tasks.push(tokio::task::spawn(
                new_node_block_header_worker(client.clone(), new_block_hash_channel.clone(), channel)
            ));
        }
        None => {}
    }


    match new_block_logs_channel {
        Some(channel) => {
            tasks.push(tokio::task::spawn(
                new_node_block_logs_worker(
                    client.clone(),
                    new_block_hash_channel.clone().subscribe().await, channel)
            ));
        }
        None => {}
    }


    match new_block_state_update_channel {
        Some(channel) => {
            tasks.push(tokio::task::spawn(
                new_node_block_state_worker(
                    client.clone(),
                    new_block_hash_channel.clone().subscribe().await,
                    channel)
            ));
        }
        None => {}
    }

    Ok(tasks)
}

#[derive(Producer)]
pub struct NodeBlockActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static
{
    client: P,
    #[producer]
    block_header_channel: Option<Broadcaster<Header>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<Block>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<BlockLogsUpdate>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
}

impl<P> NodeBlockActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static
{
    pub fn new(client: P) -> NodeBlockActor<P> {
        NodeBlockActor {
            client,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
        }
    }
}

#[async_trait]
impl<P> Actor for NodeBlockActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    async fn start(&mut self) -> ActorResult {
        new_node_block_starer(
            self.client.clone(),
            self.block_header_channel.clone(),
            self.block_with_tx_channel.clone(),
            self.block_logs_channel.clone(),
            self.block_state_update_channel.clone(),
        ).await
    }
}
