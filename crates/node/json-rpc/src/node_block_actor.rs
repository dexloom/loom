use std::marker::PhantomData;

use alloy_network::Ethereum;
use alloy_provider::Provider;
use alloy_transport::Transport;
use tokio::task::JoinHandle;

use crate::node_block_hash_worker::new_node_block_header_worker;
use crate::node_block_logs_worker::new_node_block_logs_worker;
use crate::node_block_state_worker::new_node_block_state_worker;
use crate::node_block_with_tx_worker::new_block_with_tx_worker;
use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_node_actor_config::NodeBlockActorConfig;
use loom_node_db_access::reth_node_worker_starter;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};

pub fn new_node_block_workers_starter<P, T>(
    client: P,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
) -> ActorResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    let new_header_internal_channel = Broadcaster::new(10);
    let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

    if let Some(channel) = new_block_with_tx_channel {
        tasks.push(tokio::task::spawn(new_block_with_tx_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_headers_channel {
        tasks.push(tokio::task::spawn(new_node_block_header_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_logs_channel {
        tasks.push(tokio::task::spawn(new_node_block_logs_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_state_update_channel {
        tasks.push(tokio::task::spawn(new_node_block_state_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    Ok(tasks)
}

#[derive(Producer)]
pub struct NodeBlockActor<P, T> {
    client: P,
    config: NodeBlockActorConfig,
    reth_db_path: Option<String>,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
    _t: PhantomData<T>,
}

impl<P, T> NodeBlockActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        "NodeBlockActor"
    }

    pub fn new(client: P, config: NodeBlockActorConfig) -> NodeBlockActor<P, T> {
        NodeBlockActor {
            client,
            config,
            reth_db_path: None,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
            _t: PhantomData,
        }
    }

    pub fn with_reth_db(self, reth_db_path: Option<String>) -> Self {
        Self { reth_db_path, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            block_header_channel: if self.config.block_header { Some(bc.new_block_headers_channel()) } else { None },
            block_with_tx_channel: if self.config.block_with_tx { Some(bc.new_block_with_tx_channel()) } else { None },
            block_logs_channel: if self.config.block_logs { Some(bc.new_block_logs_channel()) } else { None },
            block_state_update_channel: if self.config.block_state_update { Some(bc.new_block_state_update_channel()) } else { None },
            ..self
        }
    }
}

impl<P, T> Actor for NodeBlockActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        match &self.reth_db_path {
            // TODO: Refactor to own crate
            //RETH DB
            Some(db_path) => reth_node_worker_starter(
                self.client.clone(),
                db_path.clone(),
                self.block_header_channel.clone(),
                self.block_with_tx_channel.clone(),
                self.block_logs_channel.clone(),
                self.block_state_update_channel.clone(),
            ),
            //RPC
            None => new_node_block_workers_starter(
                self.client.clone(),
                self.block_header_channel.clone(),
                self.block_with_tx_channel.clone(),
                self.block_logs_channel.clone(),
                self.block_state_update_channel.clone(),
            ),
        }
    }
    fn name(&self) -> &'static str {
        self.name()
    }
}

#[cfg(test)]
mod test {
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::{ClientBuilder, WsConnect};
    use alloy_rpc_types::Header;
    use tokio::select;
    use tracing::{debug, error, info};

    use crate::node_block_actor::NodeBlockActorConfig;
    use crate::NodeBlockActor;
    use eyre::Result;
    use loom_core_actors::{Actor, Broadcaster, Producer};
    use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};

    #[tokio::test]
    #[ignore]
    async fn revm_worker_test() -> Result<()> {
        let _ = env_logger::builder().format_timestamp_millis().try_init();

        info!("Creating channels");
        let new_block_headers_channel: Broadcaster<MessageBlockHeader> = Broadcaster::new(10);
        let new_block_with_tx_channel: Broadcaster<MessageBlock> = Broadcaster::new(10);
        let new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate> = Broadcaster::new(10);
        let new_block_logs_channel: Broadcaster<MessageBlockLogs> = Broadcaster::new(10);

        let node_url = std::env::var("DEVNET_WS")?;

        let ws_connect = WsConnect::new(node_url);
        let client = ClientBuilder::default().ws(ws_connect).await.unwrap();
        let client = ProviderBuilder::new().on_client(client).boxed();

        let db_path = std::env::var("TEST_NODE_DB")?;

        let mut node_block_actor = NodeBlockActor::new(client.clone(), NodeBlockActorConfig::all_enabled()).with_reth_db(Some(db_path));
        match node_block_actor
            .produce(new_block_headers_channel.clone())
            .produce(new_block_with_tx_channel.clone())
            .produce(new_block_logs_channel.clone())
            .produce(new_block_state_update_channel.clone())
            .start()
        {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("Node actor started successfully")
            }
        }

        let mut new_block_rx = new_block_headers_channel.subscribe().await;
        let mut new_block_with_tx_rx = new_block_with_tx_channel.subscribe().await;
        let mut new_block_logs_rx = new_block_logs_channel.subscribe().await;
        let mut new_block_state_update_rx = new_block_state_update_channel.subscribe().await;

        for i in 1..10 {
            select! {
                msg_fut = new_block_rx.recv() => {
                    let msg : Header = msg_fut?.inner.header;
                    debug!("Block header received : {:?}", msg);
                }
                msg_fut = new_block_with_tx_rx.recv() => {
                    let msg : MessageBlock = msg_fut?;
                    debug!("Block withtx received : {:?}", msg);
                }
                msg_fut = new_block_logs_rx.recv() => {
                    let msg : MessageBlockLogs = msg_fut?;
                    debug!("Block logs received : {:?}", msg);
                }
                msg_fut = new_block_state_update_rx.recv() => {
                    let msg : MessageBlockStateUpdate = msg_fut?;
                    debug!("Block state update received : {:?}", msg);
                }

            }

            //tokio::time::sleep(Duration::new(3, 0)).await;
            println!("{i}")
        }
        Ok(())
    }
}
