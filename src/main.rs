use std::process;

use vaserve::cli;
use vaserve::server;

#[tokio::main]
async fn main() {
    let args = cli::parse_args();

    if args.help {
        cli::print_help();
        process::exit(0);
    }

    if args.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    if let Err(e) = server::start(args).await {
        eprintln!("ERROR: {}", e);
        process::exit(1);
    }
}
