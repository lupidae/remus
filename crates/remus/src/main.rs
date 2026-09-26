//! Thin entrypoint. Everything lives in the library so it can be tested.

use std::{error::Error as _, process::ExitCode};

use remus::Error;

#[tokio::main]
async fn main() -> ExitCode {
    match remus::cli::run().await {
        Ok(()) => ExitCode::SUCCESS,
        // Walking away from a prompt is an answer, not an error worth printing.
        Err(Error::Canceled) => ExitCode::from(130),
        Err(err) => {
            // Driver errors keep the useful part (auth failure, refused
            // connection, SQLSTATE) in their source chain, not in Display.
            eprintln!("remus: {err}");
            let mut cause = err.source();
            while let Some(inner) = cause {
                eprintln!("  caused by: {inner}");
                cause = inner.source();
            }
            ExitCode::FAILURE
        }
    }
}
