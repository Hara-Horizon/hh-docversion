use anyhow::Result;
use cargo_docversion::{DocversionCli, run_cli};
use clap::Parser;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Dynamically retrieve binary name and version from Cargo metadata at compile time
    let pkg_name = env!("CARGO_PKG_NAME");
    let pkg_version = env!("CARGO_PKG_VERSION");

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("{} {}", pkg_name, pkg_version);
        return Ok(());
    }

    // Dynamically check if invoked as a cargo subcommand (e.g., `cargo hh-docversion`)
    let parsed_args = if args.get(1).is_some_and(|arg| arg == pkg_name) {
        let mut shifted = vec![args[0].clone()];
        shifted.extend(args.into_iter().skip(2));
        DocversionCli::parse_from(shifted)
    } else {
        DocversionCli::parse()
    };

    run_cli(parsed_args)
}