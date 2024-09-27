use defi_blockchain::Blockchain;
use defi_entities::NodeWrapper;
use defi_pools::db_reader::{UniswapV2DBReader, UniswapV3DBReader};
use eyre::eyre;
use log::info;
use loom_actors::{Actor, ActorResult, WorkerResult};
use loom_actors_macros::Accessor;
use reth_node_api::{FullNodeComponents, NodeAddOns};
use reth_provider::StateProviderFactory;
use std::time::Instant;

async fn pool_loader_one_shot_worker<Node, AddOns>(node_wrapper: NodeWrapper<Node, AddOns>) -> WorkerResult
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    let provider = node_wrapper.node.unwrap().provider;

    load_uniswapv2::<Node>(&provider)?;
    load_uniswapv3::<Node>(&provider)?;

    Ok("Pools loaded".to_string())
}

fn load_uniswapv2<Node>(provider: &Node::Provider) -> eyre::Result<()>
where
    Node: FullNodeComponents + Clone,
{
    let now = Instant::now();

    let uniswap2_db_reader = UniswapV2DBReader::new();
    let pairs_len = uniswap2_db_reader.read_pairs_len(provider.latest()?)?;

    let chunk_size = 1000;
    let mut start = 0;
    let mut pairs = Vec::new();

    // Reading in chunks to avoid long transaction error.
    while start < pairs_len {
        let end = std::cmp::min(start + chunk_size, pairs_len);
        let chunk_pairs = uniswap2_db_reader.read_pairs(provider.latest()?, start, end)?;
        pairs.extend(chunk_pairs);
        start = end;
        info!("loaded so far: {}", pairs.len());
    }
    let elapsed = now.elapsed();
    info!("Loaded {} univ2 pairs in {:.2?} sec", pairs.len(), elapsed);
    Ok(())
}

fn load_uniswapv3<Node>(provider: &Node::Provider) -> eyre::Result<()>
where
    Node: FullNodeComponents + Clone,
{
    let now = Instant::now();
    let pools = UniswapV3DBReader::read_univ3_position_pools(provider.latest()?)?;

    let elapsed = now.elapsed();
    info!("Loaded {} univ3 pools in {:.2?} sec", pools.len(), elapsed);
    Ok(())
}

/// The one-shot actor reads all existing uniswap v2 pairs and v3 pools.
#[derive(Accessor)]
pub struct DbPoolLoaderOneShotActor<Node, AddOns>
where
    Node: FullNodeComponents,
    AddOns: NodeAddOns<Node>,
{
    node_wrapper: NodeWrapper<Node, AddOns>,
}

impl<Node, AddOns> DbPoolLoaderOneShotActor<Node, AddOns>
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    pub fn new(node_wrapper: NodeWrapper<Node, AddOns>) -> DbPoolLoaderOneShotActor<Node, AddOns> {
        DbPoolLoaderOneShotActor { node_wrapper }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { ..self }
    }
}

impl<Node, AddOns> Actor for DbPoolLoaderOneShotActor<Node, AddOns>
where
    Node: FullNodeComponents + Clone,
    AddOns: NodeAddOns<Node> + Clone,
{
    fn start_and_wait(&self) -> eyre::Result<()> {
        let rt = tokio::runtime::Runtime::new()?; // we need a different runtime to wait for the result
        let node_wrapper = self.node_wrapper.clone();
        let handle = rt.spawn(async { pool_loader_one_shot_worker(node_wrapper).await });

        self.wait(Ok(vec![handle]))?;
        rt.shutdown_background();

        Ok(())
    }
    fn start(&self) -> ActorResult {
        Err(eyre!("NEED_TO_BE_WAITED"))
    }

    fn name(&self) -> &'static str {
        "DbPoolLoaderOneShotActor"
    }
}
