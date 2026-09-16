//! Output formats. Every emitter is a pure function of the [`Schema`].
//!
//! JSON is the model itself and therefore lossless. SQL covers every object the
//! model carries. Mermaid and DBML are diagram formats and drop what they cannot
//! express; the README's fidelity table is the contract for what each keeps.

pub mod dbml;
mod ident;
pub mod mermaid;
pub mod sql;

use std::fmt;

use crate::{Error, Schema};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    Json,
    Mermaid,
    Dbml,
    Sql,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Json, Format::Mermaid, Format::Dbml, Format::Sql];

    pub fn extension(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Mermaid => "mmd",
            Format::Dbml => "dbml",
            Format::Sql => "sql",
        }
    }

    /// Diagram formats honour [`DiagramOptions`]. JSON and SQL are exhaustive by
    /// construction: collapsing a junction table or hiding a view would make them lie.
    pub fn is_diagram(&self) -> bool {
        matches!(self, Format::Mermaid | Format::Dbml)
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Format::Json => "json",
            Format::Mermaid => "mermaid",
            Format::Dbml => "dbml",
            Format::Sql => "sql",
        };
        f.write_str(name)
    }
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

pub fn render(schema: &Schema, format: &Format, options: &DiagramOptions) -> Result<String, Error> {
    match format {
        Format::Json => json(schema),
        Format::Mermaid => Ok(mermaid::render(schema, options)),
        Format::Dbml => Ok(dbml::render(schema, options)),
        Format::Sql => Ok(sql::render(schema)),
    }
}

/// The model, pretty-printed. This is the authoritative export.
pub fn json(schema: &Schema) -> Result<String, Error> {
    let mut out = serde_json::to_string_pretty(schema)?;
    out.push('\n');
    Ok(out)
}
