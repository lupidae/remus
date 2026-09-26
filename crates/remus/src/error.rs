use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(
        "no database given: run `remus` on its own to be walked through it, or pass --url\n  \
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
    #[error("cancelled")]
    Canceled,
    #[error("could not read your answer")]
    Prompt(#[from] inquire::InquireError),
}
