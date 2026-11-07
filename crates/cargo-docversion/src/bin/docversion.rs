use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cargo_docversion::DocversionCli::parse();
    cargo_docversion::run_cli(cli)
}
