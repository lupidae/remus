use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(
        "no database given: pass --url, set DATABASE_URL, or feed a payload with --input\n  \
         credential-free: remus --print-sql | psql \"$DATABASE_URL\" -Atf - | remus --input -"
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
}
