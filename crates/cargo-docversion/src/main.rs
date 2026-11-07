use clap::Parser;

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    eprintln!("[docversion debug] Raw arguments: {:?}", raw);

    let mut clean_args: Vec<String> = raw
        .into_iter()
        .skip(1)
        .filter(|arg| arg != "docversion")
        .collect();

    clean_args.insert(0, "docversion".to_string());
    eprintln!("[docversion debug] Passing to DocversionCli: {:?}", clean_args);

    let cli = match cargo_docversion::DocversionCli::try_parse_from(&clean_args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[docversion parse error]\n{}", e);
            std::process::exit(1);
        }
    };

    if let Err(err) = cargo_docversion::run_cli(cli) {
        eprintln!("[docversion error] Execution failed: {:#}", err);
        std::process::exit(1);
    }
}
