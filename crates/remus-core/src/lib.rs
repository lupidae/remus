//! The schema model and the emitters that render it.
//!
//! This crate deliberately has no I/O and no async runtime: a [`Schema`] comes
//! in as JSON (from `remus` connecting to a database, or from a user pasting the
//! output of [`INTROSPECT_SQL`]) and every format is a pure function of it. That
//! is what lets the same code later run in a server or in the browser without
//! the CLI output and the hosted output ever drifting apart.

pub mod emit;
mod error;
#[cfg(feature = "fixtures")]
pub mod fixtures;
pub mod model;

pub use error::Error;
pub use model::Schema;

/// The introspection query, byte-identical to what `remus --print-sql` prints.
///
/// It is the contract between the database and [`Schema`]: the JSON it returns
/// is exactly what [`Schema::from_json`] parses.
pub const INTROSPECT_SQL: &str = include_str!("../queries/introspect.sql");
