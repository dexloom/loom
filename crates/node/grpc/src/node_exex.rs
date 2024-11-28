use crate::node_exex_worker::node_exex_grpc_worker;
use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageMempoolDataUpdate};
use std::any::type_name;

#[derive(Producer)]
pub struct NodeExExGrpcActor {
    url: String,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
    #[producer]
    mempool_update_channel: Option<Broadcaster<MessageMempoolDataUpdate>>,
}

impl NodeExExGrpcActor {
    pub fn new(url: String) -> NodeExExGrpcActor {
        NodeExExGrpcActor {
            url,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
            mempool_update_channel: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            block_header_channel: Some(bc.new_block_headers_channel()),
            block_with_tx_channel: Some(bc.new_block_with_tx_channel()),
            block_logs_channel: Some(bc.new_block_logs_channel()),
            block_state_update_channel: Some(bc.new_block_state_update_channel()),
            mempool_update_channel: Some(bc.new_mempool_tx_channel()),
            ..self
        }
    }
}

impl Actor for NodeExExGrpcActor {
    fn start(&self) -> ActorResult {
        let handler = tokio::task::spawn(node_exex_grpc_worker(
            Some(self.url.clone()),
            self.block_header_channel.clone().unwrap(),
            self.block_with_tx_channel.clone().unwrap(),
            self.block_logs_channel.clone().unwrap(),
            self.block_state_update_channel.clone().unwrap(),
            self.mempool_update_channel.clone().unwrap(),
        ));
        Ok(vec![handler])
    }

    fn name(&self) -> &'static str {
        type_name::<Self>().rsplit("::").next().unwrap_or(type_name::<Self>())
    }
}
