use anyhow::Result;
use cargo_docversion::{DocversionCli, run_cli};
use clap::Parser;

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("hh-docversion 0.1.0");
        return Ok(());
    }

    // When invoked as `cargo hh-docversion`, Cargo passes "hh-docversion" as the first argument.
    // Strip it so Clap correctly parses the intended subcommand/flags.
    if args.len() > 1 && args[1] == "hh-docversion" {
        args.remove(1);
    }

    let cli = DocversionCli::try_parse_from(args)?;
    run_cli(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_subcommand_argument_stripping() {
        // Simulate invocation: cargo hh-docversion site --out-dir target/site
        let mut args = vec![
            "docversion".to_string(),
            "hh-docversion".to_string(),
            "site".to_string(),
            "--out-dir".to_string(),
            "target/site".to_string(),
        ];

        if args.len() > 1 && args[1] == "hh-docversion" {
            args.remove(1);
        }

        let result = DocversionCli::try_parse_from(args);
        assert!(result.is_ok(), "Failed to parse sanitized cargo subcommand args: {:?}", result.err());
    }

    #[test]
    fn test_standalone_binary_parsing() {
        // Simulate invocation: docversion site --out-dir target/site
        let args = vec![
            "docversion".to_string(),
            "site".to_string(),
            "--out-dir".to_string(),
            "target/site".to_string(),
        ];

        let result = DocversionCli::try_parse_from(args);
        assert!(result.is_ok(), "Failed to parse standalone binary args: {:?}", result.err());
    }
}