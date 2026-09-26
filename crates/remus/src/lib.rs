//! The `remus` command line: connect (or read a captured payload), build the
//! model, write one or more formats. Rendering itself lives in `remus-core`.

pub mod cli;
mod error;
pub mod format;
mod guide;
mod introspect;

pub use error::Error;
