use anyhow::Result;
use cargo_docversion::{DocversionCli, run_cli};
use clap::Parser;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("hh-docversion 0.1.0");
        return Ok(());
    }

    let cli = DocversionCli::parse();
    run_cli(cli)
}