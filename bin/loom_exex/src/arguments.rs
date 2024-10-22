use clap::{Parser, Subcommand};

/// Triggers persistence when the number of canonical blocks in memory exceeds this threshold.
pub const DEFAULT_PERSISTENCE_THRESHOLD: u64 = 2;

/// How close to the canonical head we persist blocks.
pub const DEFAULT_MEMORY_BLOCK_BUFFER_TARGET: u64 = 2;

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

    // Original RETH CLI arguments
    /// Configure persistence threshold for engine experimental.
    #[arg(long = "engine.persistence-threshold", default_value_t = DEFAULT_PERSISTENCE_THRESHOLD)]
    pub persistence_threshold: u64,

    /// Configure the target number of blocks to keep in memory.
    #[arg(long = "engine.memory-block-buffer-target", default_value_t = DEFAULT_MEMORY_BLOCK_BUFFER_TARGET)]
    pub memory_block_buffer_target: u64,
}
