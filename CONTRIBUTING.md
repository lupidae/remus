# Contributing

Thanks for looking. Issues and pull requests are welcome.

## Read this before writing code

[`CLAUDE.md`](CLAUDE.md) is the design document: every decision that is settled,
and why. It is addressed to an AI assistant, but it is the honest record, and a
change that contradicts it needs a reason rather than a preference.

The short version of what this tool is *not*, so no one writes a rejected PR:

- **No GUI, canvas, layout or diff/sync.** pgAdmin and pgModeler are better at
  visual modelling and that is fine. remus is text out, pipes in, runs in CI.
- **Four formats, no more** unless someone asks for one. Graphviz DOT and a
  Markdown data dictionary are the two we would add next.
- **Emitters are one-directional.** No DBML parser, no SQL parser, no importer.
- **No second query.** `queries/introspect.sql` is one statement, `pg_catalog`
  only, never `information_schema`, never row data.
- **`remus-core` stays free of I/O and async**, so it can compile to wasm.

## Getting set up

```bash
just check          # fmt, check, clippy -D warnings, tests — the whole gate
```

No database is needed. The emitters are tested against a committed
introspection, so `cargo test` works on a fresh clone.

For the paths that do need Postgres:

```bash
docker run -d --name remus-pg -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 postgres:16
just demo           # run against DATABASE_URL
just examples       # regenerate examples/*/out
```

Postgres **16** on purpose: `server_version` is recorded in the fixture and
printed in the DBML and SQL headers, so another major version rewrites golden
files for no schema change.

## Changing an emitter

Output is golden-file tested. When you change it on purpose:

```bash
just golden         # rewrite crates/remus/tests/expected/
git diff            # then read every line of it
```

An emitter change also wants the fidelity table in `site/index.html` updated —
that table is the contract for what each format keeps — and a unit test if you
touched a rule.

## House style

- `thiserror` enums and `?`. No `anyhow`. An error message never repeats its
  source; `main` prints the cause chain.
- Comments explain **why**. If a reviewer would ask "what does this do?",
  rewrite the code instead.
- Tests live in `#[cfg(test)] mod tests` and are named as assertions:
  `views_come_after_what_they_read`.
- `cargo +nightly fmt` (the import grouping needs nightly), `clippy -D
  warnings`, no `allow`.

CI runs all of that on Linux, macOS and Windows. `just check` locally is the
same gate minus the other two platforms.

## Releasing

Tagging `vX.Y.Z` runs everything at once: binaries for six targets, a
multi-arch image on ghcr, and `cargo publish --workspace`, which uploads the
crates in dependency order by itself. Nothing needs doing by hand and nothing
needs doing in sequence.

A crates.io version can be yanked but never deleted or reused, so the publish
job sits behind a `crates-io` GitHub Environment. Give that environment a
required reviewer if you want a human in front of it.

## Licence

By contributing you agree your work is dual licensed under
[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), matching the project. No
CLA, no copyright assignment.
