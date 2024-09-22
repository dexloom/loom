use alloy::providers::ProviderBuilder;
use clap::{CommandFactory, FromArgMatches, Parser};
use reth::args::utils::DefaultChainSpecParser;
use reth_node_ethereum::EthereumNode;

use crate::arguments::{AppArgs, Command, LoomArgs};
use defi_blockchain::Blockchain;
use loom_topology::TopologyConfig;

mod arguments;
mod loom;

fn main() -> eyre::Result<()> {
    // do not rais an error for unknown commands
    let app_args = AppArgs::from_arg_matches_mut(&mut AppArgs::command().ignore_errors(true).get_matches())?;
    match app_args.command {
        Command::Node(_) => reth::cli::Cli::<DefaultChainSpecParser, LoomArgs>::parse().run(|builder, loom_args: LoomArgs| async move {
            let topology_config = TopologyConfig::load_from_file(loom_args.config)?;

            let bc = Blockchain::new(builder.config().chain.chain.id());
            let bc_clone = bc.clone();

            let handle =
                builder.node(EthereumNode::default()).install_exex("loom-exex", |node_ctx| loom::init(node_ctx, bc_clone)).launch().await?;

            let mempool = handle.node.pool.clone();
            let ipc_provider = ProviderBuilder::new().on_builtin(handle.node.config.rpc.ipcpath.as_str()).await?;

            tokio::task::spawn(loom::mempool_worker(mempool, bc.clone()));
            tokio::task::spawn(async move {
                if let Err(e) = loom::start_loom(ipc_provider, bc, topology_config).await {
                    panic!("{}", e)
                }
            });

            handle.wait_for_node_exit().await
        }),
        Command::Remote(_loom_args) => {
            // start remote mode without exex
            todo!()
        }
    }
}
