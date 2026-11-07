use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    // If invoked directly as "cargo-docversion <subcmd>", normalize so CargoCli parses cleanly
    let cli = if args.len() > 1 && args[1] != "docversion" {
        cargo_docversion::CargoCli::parse_from(
            std::iter::once("cargo".to_string())
                .chain(std::iter::once("docversion".to_string()))
                .chain(args.into_iter().skip(1)),
        )
    } else {
        cargo_docversion::CargoCli::parse()
    };

    cargo_docversion::run_cargo_plugin(cli)
}
