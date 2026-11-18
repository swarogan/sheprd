//! `sheprd-push-relay`: see the library.

use sheprd_push_relay::{Options, Result, serve};
use std::process::ExitCode;

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args_os().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{}", sheprd_push_relay::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n\n{}", sheprd_push_relay::USAGE);
            return ExitCode::from(2);
        }
    };
    tracing_subscriber::fmt().with_target(false).init();
    let check = options.check;
    match run(options) {
        Ok(()) if check => {
            tracing::info!("the relay can start with these secrets");
            ExitCode::SUCCESS
        }
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if check => {
            tracing::error!(%error, "the relay cannot start with these secrets");
            ExitCode::FAILURE
        }
        Err(error) => {
            tracing::error!(%error, "the relay stopped");
            ExitCode::FAILURE
        }
    }
}

fn run(options: Options) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(sheprd_push_relay::Error::Runtime)?
        .block_on(serve(options))
}
