use cargo_docversion::{DocversionCli, run_cli};
use clap::Parser;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // When invoked as `cargo hh-docversion`, Cargo passes "hh-docversion" as the first argument.
    // Strip it so Clap correctly parses the intended subcommand/flags.
    if args.len() > 1 && args[1] == "hh-docversion" {
        args.remove(1);
    }

    let cli = DocversionCli::try_parse_from(args).unwrap_or_else(|e| e.exit());

    if let Err(err) = run_cli(cli) {
        eprintln!("Error: {:#}", err);
        std::process::exit(1);
    }
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
        assert!(
            result.is_ok(),
            "Failed to parse sanitized cargo subcommand args: {:?}",
            result.err()
        );
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
        assert!(
            result.is_ok(),
            "Failed to parse standalone binary args: {:?}",
            result.err()
        );
    }
}
