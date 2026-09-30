//! The `remus` command line: connect (or read a captured payload), build the
//! model, write one or more formats. Rendering itself lives in `remus-core`.

pub mod cli;
mod error;
pub mod format;
mod introspect;

#[cfg(feature = "guided")]
mod guide;

/// Without the `guided` feature there is no prompting code to link, so
/// [`cli::Cli::interactive`] is always false and this never runs. It exists so
/// the one call site needs no `cfg` of its own.
#[cfg(not(feature = "guided"))]
mod guide {
    use crate::{Error, cli::Cli};

    pub async fn run(_cli: &Cli) -> Result<(), Error> {
        Err(Error::NoSource)
    }
}

pub use error::Error;
