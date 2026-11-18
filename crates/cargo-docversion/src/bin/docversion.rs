use anyhow::Result;
use cargo_docversion::{DocversionCli, run_cli};
use clap::Parser;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("hh-docversion 0.1.0");
        return Ok(());
    }

    let parsed_args = if args.get(1).is_some_and(|arg| arg == "hh-docversion") {
        let mut shifted = vec![args[0].clone()];
        shifted.extend(args.into_iter().skip(2));
        DocversionCli::parse_from(shifted)
    } else {
        DocversionCli::parse()
    };

    run_cli(parsed_args)
}