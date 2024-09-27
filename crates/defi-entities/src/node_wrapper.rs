use alloy_rpc_types::BlockId;
use eyre::eyre;
use reth_node_builder::{FullNode, FullNodeComponents, NodeAddOns};
use reth_provider::{StateProviderBox, StateProviderFactory};
use reth_revm::database::StateProviderDatabase;
use revm::db::CacheDB;

#[derive(Clone)]
pub struct NodeWrapper<Node: FullNodeComponents, AddOns: NodeAddOns<Node>> {
    pub node: Option<FullNode<Node, AddOns>>,
}

impl<Node, AddOns> NodeWrapper<Node, AddOns>
where
    Node: FullNodeComponents,
    AddOns: NodeAddOns<Node>,
{
    pub fn new(node: Option<FullNode<Node, AddOns>>) -> Self {
        Self { node }
    }

    pub fn is_exex(&self) -> bool {
        self.node.is_some()
    }

    pub fn fork_db_reth(&self, block_id: BlockId) -> eyre::Result<CacheDB<StateProviderDatabase<StateProviderBox>>> {
        if let Some(node) = self.node.as_ref() {
            let state = node.provider.state_by_block_id(block_id)?;
            return Ok(CacheDB::new(StateProviderDatabase::new(state)));
        }
        Err(eyre!("Node is not set"))
    }
}
