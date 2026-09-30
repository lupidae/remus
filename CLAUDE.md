# remus

Postgres schema export CLI: one introspection query → JSON model → Mermaid,
DBML, SQL DDL. Crates: `remus-core` (model, the introspection query, the
`Emitter` trait, JSON; no I/O), one crate per emitter (`remus-mermaid`,
`remus-dbml`, `remus-sql`), and `remus` (clap CLI, tokio-postgres, and the
format registry that is the only place naming every emitter). Read the code for
structure; this file holds what the code cannot tell you.

## Prior art, so we don't rebuild it

pgAdmin draws an ERD from a live database. pgModeler is a Postgres-native
desktop modeller with reverse engineering, diff and sync, since 2013. Both are
better than us at visual modelling and that is fine. Do not propose a GUI, a
canvas, layout, or diff/sync features. What is ours:

1. Text-first output that pipes, diffs in a PR and renders on GitHub.
2. Postgres fidelity in the model (enums, domains, composites, RLS, partitions,
   generated columns, views) instead of generic boxes.
3. `--conceptual`: the junction-table collapse toward an N:N model. Nothing
   else on the market does it.
4. Unix shape: one static binary, credential-free path via `--print-sql`.

A proposed feature either strengthens one of these or belongs in pgModeler.

## Decisions already made (change with a reason, not a preference)

- **`queries/introspect.sql` is a product surface.** Embedded with
  `include_str!`, printed verbatim by `--print-sql`, runnable in psql as-is.
  One statement. Do not split it, generate it from Rust, or add a second query.
- **It pins `search_path = ''` inside the statement** (the `cfg` CTE, referenced
  by the final `FROM`). Every catalog deparse function then schema-qualifies
  names, so the payload does not depend on who ran it and the emitted DDL is
  unambiguous. Keep `FROM cfg` at the end; without it the CTE may never run.
- **pg_catalog only, never information_schema, never row data.** A reader must
  be able to verify that by reading the SQL.
- **JSON is authoritative, every other format is lossy**, and the fidelity table
  in `site/index.html` is the contract. Update it with every emitter change. The
  README stays short and links to it; long-form documentation lives on the site.
- **Emitters are one-directional.** No DBML parser, no SQL parser, no importer:
  anything read back would be a poorer model than the catalog gives.
- **Four formats, no more** unless someone asks. Graphviz DOT and a Markdown
  data dictionary are the two we would add next; PlantUML, D2 and native SVG
  are out.
- **SQL output is not pg_dump.** It covers modelled objects only and its header
  says what is excluded. Do not add functions, triggers, grants or sequence
  state to the model to "complete" it. Foreign keys are emitted as
  `ALTER TABLE` after every `CREATE TABLE` because circular references are legal.
- **Cardinality is what the catalog proves.** Parent end `||` when all FK
  columns are NOT NULL, else `|o`. Child end `o|` when the FK columns are
  unique, else `o{`. The child end is never `||` or `|{`: no catalog fact says a
  parent must have a child. (The original draft had nullability on the child
  end; that was wrong.)
- **`is_junction` is the only heuristic** and stays conservative: exactly two
  FKs, every column an FK column, generated, or in `HOUSEKEEPING`. Polymorphic
  associations are never inferred.
- **Leaf partitions are not entities**; they are listed under `partitions`.
  One box per partitioned table, not forty.
- **Identity is qualified names, never OIDs.** OIDs change on restore and differ
  between environments. Nothing physical that varies between two restores of
  the same schema belongs in the model (that is why `attnum` is not carried).
- **The guided flow is behind the default-on `guided` feature.** A CI image can
  build with `--no-default-features` and link no prompting code (14 crates), and
  every flag still works. Nothing prompts unless stdin and stderr are both
  terminals, `CI` is unset and `--no-input` was not passed: a PTY is not proof
  that anyone is watching.
- **`remus-core` has no I/O and no async.** It must stay compilable to wasm
  later. No `tokio`, no `std::fs` in core.
- **One crate per emitter.** Each depends on `remus-core` and never on another
  emitter, so the compiler — not a convention — keeps formats from reaching into
  each other, and a wasm build can link one format alone. The dispatch table
  cannot live in core (that would be a cycle); it is `crates/remus/src/format.rs`,
  next to the clap enum. Each crate exposes an honest free `render` (infallible,
  and without `DiagramOptions` where they mean nothing) plus a unit-type
  `Emitter` impl for the CLI.

## Not in scope right now

- Overlay files (positions, notes, soft FKs keyed by qualified name), a server,
  realtime, and the storage design from the original conversation. The model is
  shaped to allow them later; do not start building them.
- TLS with private CAs. Public roots only; document the `--print-sql` path.

## Pitfalls

- `pg_get_*def` output is deparsed, not the user's text. After a round trip
  Postgres may re-deparse an equivalent expression differently (seen on an
  `ARRAY[...]::text[]` predicate). Not a bug.
- `pg_inherits` also links partitioned indexes to their partitions; the
  `partitions` CTE filters on relkind for that reason.
- Indexes backing PK/UNIQUE/EXCLUDE constraints are excluded from `indexes`;
  emitting both would create the same name twice.
- Policies naming roles and defaults calling user functions make the SQL output
  depend on objects we do not create. Documented in the header, by design.
- Mermaid attribute types allow only `[A-Za-z0-9_()\[\]]`; `token()` sanitises.
  Mermaid strings cannot contain double quotes.

## Conventions (aligned with the Vault codebase)

- Errors: `thiserror` enums, `?` everywhere, no `anyhow`. Messages do not repeat
  their source; `main` prints the cause chain.
- Comments explain why, never what. If a reviewer would ask "what does this
  do?", rewrite the code instead.
- Tests inline in `#[cfg(test)] mod tests`, named as assertions
  (`views_come_after_what_they_read`). Emitter output is golden-file tested
  against `crates/remus-core/fixtures/showcase.json` (it lives with the query
  that produced it, behind the `fixtures` feature); the rendered expectations are
  `crates/remus/tests/expected/`, because only the CLI crate sees every emitter.
  Regenerate the fixture from `showcase.sql` when the query changes, then
  `just golden` and review.
- `cargo +nightly fmt` (grouped imports), `clippy -D warnings`, no `allow`.
- Every emitter change: fidelity table on the site, golden files, a unit test if
  it touches a rule.

## Verify

```bash
just check
just demo                                  # needs DATABASE_URL
psql "$DATABASE_URL" -Atf crates/remus-core/queries/introspect.sql | remus --input - -f sql
```
