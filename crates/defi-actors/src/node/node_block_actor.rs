use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_rpc_types::{Block, Header};
use alloy_transport::Transport;
use tokio::task::JoinHandle;

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_events::{BlockLogs, BlockStateUpdate};
use loom_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_actors_macros::Producer;

use crate::node::node_block_hash_worker::new_node_block_header_worker;
use crate::node::node_block_logs_worker::new_node_block_logs_worker;
use crate::node::node_block_state_worker::new_node_block_state_worker;
use crate::node::node_block_with_tx_worker::new_block_with_tx_worker;
use crate::node::reth_worker::reth_node_worker_starter;

pub fn new_node_block_starer<P, T, N>(
    client: P,
    new_block_headers_channel: Option<Broadcaster<Header>>,
    new_block_with_tx_channel: Option<Broadcaster<Block>>,
    new_block_logs_channel: Option<Broadcaster<BlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
) -> ActorResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let new_block_hash_channel = Broadcaster::new(10);
    let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

    if let Some(channel) = new_block_with_tx_channel {
        tasks.push(tokio::task::spawn(new_block_with_tx_worker(client.clone(), new_block_hash_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_headers_channel {
        tasks.push(tokio::task::spawn(new_node_block_header_worker(client.clone(), new_block_hash_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_logs_channel {
        tasks.push(tokio::task::spawn(new_node_block_logs_worker(client.clone(), new_block_hash_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_state_update_channel {
        tasks.push(tokio::task::spawn(new_node_block_state_worker(client.clone(), new_block_hash_channel.clone(), channel)));
    }

    Ok(tasks)
}

#[derive(Producer)]
pub struct NodeBlockActor<P, T, N> {
    client: P,
    reth_db_path: Option<String>,
    #[producer]
    block_header_channel: Option<Broadcaster<Header>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<Block>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<BlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> NodeBlockActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        "NodeBlockActor"
    }

    pub fn new(client: P) -> NodeBlockActor<P, T, N> {
        NodeBlockActor {
            client,
            reth_db_path: None,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
            _t: PhantomData,
            _n: PhantomData,
        }
    }

    pub fn with_reth_db(self, reth_db_path: Option<String>) -> Self {
        Self { reth_db_path, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            block_header_channel: Some(bc.new_block_headers_channel()),
            block_with_tx_channel: Some(bc.new_block_with_tx_channel()),
            block_logs_channel: Some(bc.new_block_logs_channel()),
            block_state_update_channel: Some(bc.new_block_state_update_channel()),
            ..self
        }
    }
}

impl<P, T, N> Actor for NodeBlockActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        match &self.reth_db_path {
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
            None => new_node_block_starer(
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
    use alloy_rpc_types::{Block, Header};
    use log::{debug, error, info};
    use tokio::select;

    use defi_events::{BlockLogs, BlockStateUpdate};
    use eyre::Result;
    use loom_actors::{Actor, Broadcaster, Producer};

    use crate::NodeBlockActor;

    #[tokio::test]
    #[ignore]
    async fn revm_worker_test() -> Result<()> {
        let _ = env_logger::builder().format_timestamp_millis().try_init();

        info!("Creating channels");
        let new_block_headers_channel: Broadcaster<Header> = Broadcaster::new(10);
        let new_block_with_tx_channel: Broadcaster<Block> = Broadcaster::new(10);
        let new_block_state_update_channel: Broadcaster<BlockStateUpdate> = Broadcaster::new(10);
        let new_block_logs_channel: Broadcaster<BlockLogs> = Broadcaster::new(10);

        let node_url = std::env::var("DEVNET_WS")?;

        let ws_connect = WsConnect::new(node_url);
        let client = ClientBuilder::default().ws(ws_connect).await.unwrap();
        let client = ProviderBuilder::new().on_client(client).boxed();

        let db_path = std::env::var("TEST_NODE_DB")?;

        let mut node_block_actor = NodeBlockActor::new(client.clone()).with_reth_db(Some(db_path));
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
                    let msg : Header = msg_fut.unwrap();
                    debug!("Block header received : {:?}", msg);
                }
                msg_fut = new_block_with_tx_rx.recv() => {
                    let msg : Block = msg_fut.unwrap();
                    debug!("Block withtx received : {:?}", msg);
                }
                msg_fut = new_block_logs_rx.recv() => {
                    let msg : BlockLogs = msg_fut.unwrap();
                    debug!("Block logs received : {:?}", msg);
                }
                msg_fut = new_block_state_update_rx.recv() => {
                    let msg : BlockStateUpdate = msg_fut.unwrap();
                    debug!("Block state update received : {:?}", msg);
                }

            }

            //tokio::time::sleep(Duration::new(3, 0)).await;
            println!("{i}")
        }
        Ok(())
    }
}
