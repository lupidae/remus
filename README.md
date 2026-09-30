<p align="center">
  <img src="site/logo.png" alt="remus" width="132">
</p>

<h3 align="center">Your Postgres schema, as text.</h3>

<p align="center">
  Mermaid · DBML · SQL DDL · JSON, from one static binary.<br>
  <a href="https://remus.lupidae.com"><b>remus.lupidae.com</b></a>
</p>

---

## Try it

Take a binary from the
[latest release](https://github.com/lupidae/remus/releases/latest) — macOS,
Linux (static, musl) and Windows, no toolchain needed. Or:

```bash
cargo install remus                        # any platform with Rust
docker run --rm ghcr.io/lupidae/remus --help
```

Then:

```bash
remus
```

That is the whole thing. `remus` on its own asks four questions and writes the files:

```
  remus  your Postgres schema, as text

? Database  postgres://localhost/app
  ✓ app · 14 tables, 2 views

? Schemas   public
? Formats   mermaid, sql, dbml, json
? Diagram   physical
? Folder    schema

  ✓ schema/schema.mmd    2.1 kB
  ✓ schema/schema.sql    8.4 kB
  ✓ schema/schema.dbml   4.6 kB
  ✓ schema/schema.json  36.0 kB

  next time, in one line:
  remus -u "$DATABASE_URL" -f json,mermaid,dbml,sql --out-dir schema
```

Paste `schema.mmd` into any GitHub issue or README and it renders as a diagram.
Paste `schema.dbml` into [dbdiagram.io](https://dbdiagram.io). Replay `schema.sql`
into an empty database.

## Or skip the questions

```bash
remus -u postgres://localhost/app -f all --out-dir docs/schema
remus -u postgres://localhost/app > schema.mmd
```

| | |
|---|---|
| `-f`, `--format` | `mermaid`, `dbml`, `sql`, `json`, `all` |
| `-s`, `--schema` | keep only these schemas |
| `-c`, `--conceptual` | junction tables collapse into many-to-many |
| `--views` | include views and materialized views |
| `--no-attributes` | boxes and lines, no columns |
| `--out-dir`, `--out` | write files instead of stdout |
| `--no-input` | never ask, fail instead — implied when `CI` is set |

`-u` falls back to `DATABASE_URL`. `remus --help` has the rest.

Nothing is ever asked without a terminal, so CI is safe by default. For a binary
that links no prompting code at all, build with
`--no-default-features`.

## Without handing over credentials

```bash
remus --print-sql | psql "$DATABASE_URL" -Atf - > schema.json
remus -i schema.json -f all --out-dir docs/schema
```

One statement against `pg_catalog`. Never `information_schema`, never a row of your
data. Read it before you run it.

## What it keeps

Enums, domains, composite types, partitioned tables, row level security, identity and
generated columns, views and what they read: they survive as themselves instead of
being flattened into generic boxes. JSON loses nothing; the
[fidelity table](https://remus.lupidae.com/#fidelity) says what each
other format keeps.

- **[The website](https://remus.lupidae.com)** — every output side by
  side, rendered, with how it works.
- **[examples/](examples/README.md)** — two sample schemas, every format committed.

## Development

```bash
just check    # fmt, check, clippy -D warnings, tests
just golden   # accept emitter output changes, then review the diff
just demo     # run against DATABASE_URL, everything under ./out
just examples # regenerate examples/*/out from each schema.sql
just site     # refresh the landing page, `just site-deploy` publishes it
```

No database is needed to run the tests: emitters are golden-file tested against a
committed introspection of `showcase.sql`. Each format is its own crate over the model
in `remus-core`, and none of them depends on another.

## Prior art

[pgAdmin](https://www.pgadmin.org) draws an ERD from a live database and
[pgModeler](https://pgmodeler.io) is a mature Postgres-native modeller with diff and
sync. Both are better places to *draw*. remus is the Unix-shaped complement: text out,
pipes in, runs in CI.

MIT or Apache-2.0.
