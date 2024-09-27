use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    Node(LoomArgsNode),
    Remote(LoomArgs),
}

#[derive(Parser, Debug)]
#[command(name="Loom", version, about, long_about = None)]
pub struct AppArgs {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Parser, Debug)]
pub struct LoomArgsNode {}

#[derive(Parser, Debug)]
pub struct LoomArgs {
    #[arg(long, default_value = "config.toml")]
    pub loom_config: String,
}
