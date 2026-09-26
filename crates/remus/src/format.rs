//! The format registry: the one place that knows which emitters are linked in.
//!
//! It cannot live in `remus-core`, because every emitter crate depends on core
//! and a dispatch table there would close the cycle. The CLI is the assembly
//! point, which is also where `--format` is parsed, so adding a format is one
//! dependency and one match arm here.

use std::fmt;

use remus_core::emit::{Emitter, Json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Json,
    Mermaid,
    Dbml,
    Sql,
}

impl Format {
    /// Canonical order, used to expand `--format all` and to lay out `--out-dir`.
    pub const ALL: [Format; 4] = [Format::Json, Format::Mermaid, Format::Dbml, Format::Sql];

    /// The guided flow asks in order of what people come for, which is not the
    /// canonical order the outputs are written in.
    pub const MENU: [Format; 4] = [Format::Mermaid, Format::Sql, Format::Dbml, Format::Json];

    pub fn emitter(&self) -> Box<dyn Emitter> {
        match self {
            Format::Json => Box::new(Json),
            Format::Mermaid => Box::new(remus_mermaid::Mermaid),
            Format::Dbml => Box::new(remus_dbml::Dbml),
            Format::Sql => Box::new(remus_sql::Sql),
        }
    }

    /// What the format is for, in one line, for the guided menu.
    pub fn purpose(&self) -> &'static str {
        match self {
            Format::Json => "the whole model, the only lossless format",
            Format::Mermaid => "ER diagram, renders on GitHub",
            Format::Dbml => "for dbdiagram.io, DrawSQL, ChartDB",
            Format::Sql => "DDL that replays into an empty database",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.emitter().name())
    }
}
