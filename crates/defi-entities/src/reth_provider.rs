use alloy_rpc_types::BlockId;
use reth_chainspec::ChainSpecBuilder;
use reth_db::mdbx::DatabaseArguments;
use reth_db::{open_db_read_only, ClientVersion, DatabaseEnv};
use reth_node_builder::rpc::RethRpcAddOns;
use reth_node_builder::{FullNode, FullNodeComponents, NodeTypesWithDBAdapter};
use reth_node_ethereum::EthereumNode;
use reth_provider::providers::StaticFileProvider;
use reth_provider::{ProviderFactory, ProviderResult, StateProviderBox, StateProviderFactory};
use reth_revm::database::StateProviderDatabase;
use revm::db::CacheDB;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct RethAdapter<Node: FullNodeComponents, AddOns: RethRpcAddOns<Node>> {
    pub node: Option<FullNode<Node, AddOns>>,
    pub factory: Option<ProviderFactory<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>>,
}

impl<Node, AddOns> RethAdapter<Node, AddOns>
where
    Node: FullNodeComponents,
    AddOns: RethRpcAddOns<Node>,
{
    pub fn new() -> Self {
        Self { node: None, factory: None }
    }

    pub fn new_with_node(node: FullNode<Node, AddOns>) -> Self {
        Self { node: Some(node), factory: None }
    }

    pub fn new_with_db_path(db_path: PathBuf) -> eyre::Result<Self> {
        let db = Arc::new(open_db_read_only(db_path.join("db").as_path(), DatabaseArguments::new(ClientVersion::default()))?);
        let spec = Arc::new(ChainSpecBuilder::mainnet().build());
        let factory = ProviderFactory::<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>::new(
            db.clone(),
            spec.clone(),
            StaticFileProvider::read_only(db_path.join("static_files"), true)?,
        );
        Ok(Self { node: None, factory: Some(factory) })
    }

    fn fork_db_reth(&self, block_id: BlockId) -> eyre::Result<CacheDB<StateProviderDatabase<StateProviderBox>>> {
        let state = self.node.as_ref().unwrap().provider.state_by_block_id(block_id)?;
        Ok(CacheDB::new(StateProviderDatabase::new(state)))
    }

    pub fn latest(&self) -> ProviderResult<StateProviderBox> {
        if let Some(node) = self.node.as_ref() {
            node.provider.latest()
        } else {
            self.factory.as_ref().unwrap().latest()
        }
    }
}
