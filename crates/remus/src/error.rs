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
    /// Ctrl-C or Esc in the guided flow: the user's own decision, not a failure
    /// to report.
    #[cfg(feature = "guided")]
    #[error("cancelled")]
    Canceled,
    #[cfg(feature = "guided")]
    #[error("could not read your answer")]
    Prompt(#[from] inquire::InquireError),
}
