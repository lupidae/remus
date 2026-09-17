//! What an output format is, and the one format that is the model itself.
//!
//! JSON lives here because it is the model's own serialisation and therefore
//! lossless. Every other format is its own crate (`remus-mermaid`, `remus-dbml`,
//! `remus-sql`), each depending on this one and never on another emitter, so the
//! compiler enforces that no format can reach into another's rendering. The
//! README's fidelity table is the contract for what each keeps.

pub mod ident;

use crate::{Error, Schema};

/// One output format.
///
/// Implementors are unit types: the rendering lives in each emitter crate's free
/// `render` function, with the honest signature for that format (infallible, and
/// without [`DiagramOptions`] where they mean nothing). This trait exists only so
/// the CLI can hold every linked format as one `dyn Emitter`.
pub trait Emitter {
    /// The name the CLI exposes, e.g. `mermaid`.
    fn name(&self) -> &'static str;

    /// File extension, without the dot.
    fn extension(&self) -> &'static str;

    /// Diagram formats honour [`DiagramOptions`]. JSON and SQL are exhaustive by
    /// construction: collapsing a junction table or hiding a view would make
    /// them lie.
    fn is_diagram(&self) -> bool {
        false
    }

    fn render(&self, schema: &Schema, options: &DiagramOptions) -> Result<String, Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramOptions {
    /// Collapse pure junction tables into N:N relationships.
    pub conceptual: bool,
    /// Emit columns. Off gives a boxes-and-lines overview.
    pub attributes: bool,
    /// Include views and materialized views.
    pub views: bool,
}

impl Default for DiagramOptions {
    fn default() -> Self {
        Self {
            conceptual: false,
            attributes: true,
            views: false,
        }
    }
}

pub struct Json;

impl Emitter for Json {
    fn name(&self) -> &'static str {
        "json"
    }

    fn extension(&self) -> &'static str {
        "json"
    }

    fn render(&self, schema: &Schema, _options: &DiagramOptions) -> Result<String, Error> {
        json(schema)
    }
}

/// The model, pretty-printed. This is the authoritative export.
pub fn json(schema: &Schema) -> Result<String, Error> {
    let mut out = serde_json::to_string_pretty(schema)?;
    out.push('\n');
    Ok(out)
}
