use clap::Parser;

fn main() {
    let raw: Vec<String> = std::env::args().collect();

    // Cargo passes [".../cargo-docversion", "docversion", ...].
    // Strip the binary name (arg 0) and the forwarded subcommand "docversion" (arg 1) if present.
    let mut clean_args: Vec<String> = Vec::with_capacity(raw.len());
    clean_args.push(raw.first().cloned().unwrap_or_else(|| "docversion".to_string()));

    let mut iter = raw.into_iter().skip(1);
    if let Some(first) = iter.next() {
        if first != "docversion" {
            clean_args.push(first);
        }
    }
    clean_args.extend(iter);

    // try_parse_from returns clap::Error for --help and --version as well.
    // e.exit() prints the text to stdout/stderr and exits with code 0 (for help/version)
    // or code 2 / non-zero for actual syntax errors.
    let cli = match cargo_docversion::DocversionCli::try_parse_from(&clean_args) {
        Ok(c) => c,
        Err(e) => {
            e.exit();
        }
    };

    if let Err(err) = cargo_docversion::run_cli(cli) {
        eprintln!("[docversion error] Execution failed: {:#}", err);
        std::process::exit(1);
    }
}