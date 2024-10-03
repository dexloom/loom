use crate::arguments::{AppArgs, Command, LoomArgs};
use alloy::providers::ProviderBuilder;
use clap::{CommandFactory, FromArgMatches, Parser};
use defi_actors::{mempool_worker, NodeBlockActorConfig};
use defi_blockchain::Blockchain;
use defi_entities::RethAdapter;
use loom_topology::TopologyConfig;
use reth_node_core::args::utils::EthereumChainSpecParser;
use reth_node_ethereum::EthereumNode;

mod arguments;
mod loom;

fn main() -> eyre::Result<()> {
    // ignore arguments used by reth
    let app_args = AppArgs::from_arg_matches_mut(&mut AppArgs::command().ignore_errors(true).get_matches())?;
    match app_args.command {
        Command::Node(_) => reth::cli::Cli::<EthereumChainSpecParser, LoomArgs>::parse().run(|builder, loom_args: LoomArgs| async move {
            let topology_config = TopologyConfig::load_from_file(loom_args.loom_config)?;

            let bc = Blockchain::new(builder.config().chain.chain.id());
            let bc_clone = bc.clone();

            let handle = builder
                .node(EthereumNode::default())
                .install_exex("loom-exex", |node_ctx| loom::init(node_ctx, bc_clone, NodeBlockActorConfig::default()))
                .launch()
                .await?;

            let reth_adapter = RethAdapter::new_with_node(handle.node.clone());

            let mempool = handle.node.pool.clone();
            let ipc_provider = ProviderBuilder::new().on_builtin(handle.node.config.rpc.ipcpath.as_str()).await?;

            tokio::task::spawn(mempool_worker(mempool, bc.clone()));
            tokio::task::spawn(loom::start_loom(ipc_provider, bc, topology_config, reth_adapter));

            handle.wait_for_node_exit().await
        }),
        Command::Remote(_loom_args) => {
            // start remote mode without exex
            todo!()
        }
    }
}
