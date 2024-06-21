use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(short, long)]
    pub save: bool,
    
    #[arg(value_name = "FILE", help = "Input file")]
    pub file: String,
}

