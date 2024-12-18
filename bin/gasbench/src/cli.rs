use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(short, long)]
    pub save: bool,

    #[arg(short, long)]
    pub anvil: bool,

    #[arg(short, long)]
    pub filter: Option<String>,

    #[arg(value_name = "File", help = "File name")]
    pub file: Option<String>,
}
