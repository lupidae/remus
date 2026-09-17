//! Captured introspection payloads, for tests in this workspace.
//!
//! `showcase.json` is what `queries/introspect.sql` returns for
//! `fixtures/showcase.sql`, a synthetic schema exercising every modelled
//! feature. It lives in this crate because it is the output of this crate's
//! query, and behind the `fixtures` feature so no binary that merely renders a
//! schema carries it. Regenerate with `just fixture`.

pub const SHOWCASE: &str = include_str!("../fixtures/showcase.json");
