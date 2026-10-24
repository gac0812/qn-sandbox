//! `qn` executable entry point.

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = qn_cli::Cli::parse();
    let mut stdout = std::io::stdout().lock();

    match qn_cli::run(&cli, &mut stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}", qn_cli::render_error(&error));
            ExitCode::FAILURE
        }
    }
}
