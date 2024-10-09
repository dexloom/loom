use crate::arguments::{AppArgs, Command, LoomArgs};
use alloy::providers::ProviderBuilder;
use clap::{CommandFactory, FromArgMatches, Parser};
use defi_actors::{mempool_worker, NodeBlockActorConfig};
use defi_blockchain::Blockchain;
use defi_entities::RethAdapter;
use loom_topology::TopologyConfig;
use reth_node_core::args::utils::EthereumChainSpecParser;
use reth_node_ethereum::EthereumNode;
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
        Command::Node(_) => reth::cli::Cli::<EthereumChainSpecParser, LoomArgs>::parse().run(|builder, loom_args: LoomArgs| async move {
            let topology_config = TopologyConfig::load_from_file(loom_args.loom_config)?;

            let bc = Blockchain::new(builder.config().chain.chain.id());
            let bc_clone = bc.clone();

            let handle = builder
                .node(EthereumNode::default())
                .install_exex("loom-exex", |node_ctx| loom::init(node_ctx, bc_clone, NodeBlockActorConfig::all_enabled()))
                .launch()
                .await?;

            let reth_adapter = RethAdapter::new_with_node(handle.node.clone());

            let mempool = handle.node.pool.clone();
            let ipc_provider = ProviderBuilder::new().on_builtin(handle.node.config.rpc.ipcpath.as_str()).await?;

            tokio::task::spawn(loom::start_loom(ipc_provider, bc.clone(), topology_config, reth_adapter));
            tokio::task::spawn(mempool_worker(mempool, bc));

            handle.wait_for_node_exit().await
        }),
        Command::Remote(_loom_args) => {
            // start remote mode without exex
            todo!()
        }
    }
}
