use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// The payload is not a schema this version understands. Usually a hand-edited
    /// file or the output of an older `--print-sql`.
    #[error("invalid schema payload")]
    Payload(#[from] serde_json::Error),
}
