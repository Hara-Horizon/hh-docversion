use clap::Parser;

fn main() -> anyhow::Result<()> {
    let raw: Vec<String> = std::env::args().collect();
    eprintln!("[docversion debug] Standalone raw: {:?}", raw);
    let cli = cargo_docversion::DocversionCli::parse();
    cargo_docversion::run_cli(cli)
}
