use crate::arguments::{AppArgs, Command, LoomArgs};
use alloy::providers::ProviderBuilder;
use clap::{CommandFactory, FromArgMatches, Parser};
use defi_actors::{mempool_worker, NodeBlockActorConfig};
use defi_blockchain::Blockchain;
use loom_topology::TopologyConfig;
use reth::builder::engine_tree_config::TreeConfig;
use reth::builder::EngineNodeLauncher;
use reth::chainspec::EthereumChainSpecParser;
use reth::cli::Cli;
use reth_node_ethereum::node::EthereumAddOns;
use reth_node_ethereum::EthereumNode;
use reth_provider::providers::BlockchainProvider2;
use tracing::error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

mod arguments;
mod loom;

fn main() -> eyre::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let fmt_layer = fmt::Layer::default().with_thread_ids(true).with_file(true).with_line_number(true).with_filter(env_filter);
    tracing_subscriber::registry().with(fmt_layer).init();

    // ignore arguments used by reth
    let app_args = AppArgs::from_arg_matches_mut(&mut AppArgs::command().ignore_errors(true).get_matches())?;
    match app_args.command {
        Command::Node(_) => Cli::<EthereumChainSpecParser, LoomArgs>::parse().run(|builder, loom_args: LoomArgs| async move {
            let topology_config = TopologyConfig::load_from_file(loom_args.loom_config.clone())?;

            let bc = Blockchain::new(builder.config().chain.chain.id());
            let bc_clone = bc.clone();

            let engine_tree_config = TreeConfig::default()
                .with_persistence_threshold(loom_args.persistence_threshold)
                .with_memory_block_buffer_target(loom_args.memory_block_buffer_target);
            let handle = builder
                .with_types_and_provider::<EthereumNode, BlockchainProvider2<_>>()
                .with_components(EthereumNode::components())
                .with_add_ons(EthereumAddOns::default())
                .install_exex("loom-exex", |node_ctx| loom::init(node_ctx, bc_clone, NodeBlockActorConfig::all_enabled()))
                .launch_with_fn(|builder| {
                    let launcher = EngineNodeLauncher::new(builder.task_executor().clone(), builder.config().datadir(), engine_tree_config);
                    builder.launch_with(launcher)
                })
                .await?;

            let mempool = handle.node.pool.clone();
            let ipc_provider = ProviderBuilder::new().on_builtin(handle.node.config.rpc.ipcpath.as_str()).await?;
            let bc_clone = bc.clone();
            tokio::task::spawn(async move {
                if let Err(e) = loom::start_loom(ipc_provider, bc_clone, topology_config, loom_args.loom_config.clone()).await {
                    error!("Error starting loom: {:?}", e);
                }
            });
            tokio::task::spawn(mempool_worker(mempool, bc));

            handle.node_exit_future.await
        }),
        Command::Remote(_loom_args) => {
            // start remote mode without exex
            todo!()
        }
    }
}
