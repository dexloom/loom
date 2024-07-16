use alloy::providers::ProviderBuilder;
use reth_node_ethereum::EthereumNode;

use defi_blockchain::Blockchain;
use loom_topology::TopologyConfig;

mod loom;

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let topology_config = TopologyConfig::load_from_file("config.toml".to_string())?;

        let chain_id = builder.config().chain.chain.id();
        let bc = Blockchain::new(chain_id as i64);

        let bc_clone = bc.clone();

        let handle = builder
            .node(EthereumNode::default())
            .install_exex("loom-exex", |node_ctx| async move { loom::init(node_ctx, bc_clone).await })
            .launch()
            .await?;

        let mempool = handle.node.pool.clone();
        let ipc_provider = ProviderBuilder::new().on_builtin(handle.node.config.rpc.ipcpath.as_str()).await?;

        tokio::task::spawn(loom::mempool_worker(mempool, bc.clone()));
        tokio::task::spawn(async move {
            if let Err(e) = loom::start_loom(ipc_provider, bc, topology_config).await {
                panic!("{}", e)
            }
        });

        handle.wait_for_node_exit().await
    })
}
