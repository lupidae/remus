use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    // Only reachable when no guided run was possible, so it must not suggest one.
    #[error(
        "no database given: pass --url, set DATABASE_URL, or read a payload with --input\n  \
         credential-free: remus --print-sql | psql \"$DATABASE_URL\" -Atf - | remus -i -"
    )]
    NoSource,
    #[error("{count} formats requested; writing more than one needs --out-dir")]
    NeedsOutDir { count: usize },
    #[error("could not read {}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("could not write {}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("database error")]
    Database(#[from] tokio_postgres::Error),
    #[error(transparent)]
    Schema(#[from] remus_core::Error),
    #[cfg(not(feature = "push"))]
    #[error("this build cannot push: install with `cargo install remus --features push`")]
    PushUnavailable,
    #[cfg(feature = "push")]
    #[error("could not reach romulus")]
    Push(#[source] ureq::Error),
    #[cfg(feature = "push")]
    #[error("romulus refused the push: {0}")]
    PushRefused(String),
    #[cfg(feature = "push")]
    #[error("romulus sent an answer remus does not understand")]
    PushAnswer,
    #[cfg(feature = "push")]
    #[error("the push was interrupted")]
    PushInterrupted,
    /// Ctrl-C or Esc in the guided flow: the user's own decision, not a failure
    /// to report.
    #[cfg(feature = "guided")]
    #[error("cancelled")]
    Canceled,
    #[cfg(feature = "guided")]
    #[error("could not read your answer")]
    Prompt(#[from] inquire::InquireError),
}
