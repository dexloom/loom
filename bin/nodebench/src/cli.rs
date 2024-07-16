use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cli {
    pub endpoint: Vec<String>,
}
