# remus

**https://remus.lefortlucas1.workers.dev**

Export a PostgreSQL schema as **JSON**, **Mermaid**, **DBML** or **SQL DDL**, from one
static binary. Postgres-native: enums, domains, composite types, partitions,
generated and identity columns, row level security, views and materialized views
are first-class, not flattened to generic boxes.

```bash
# one format to stdout (Mermaid by default)
remus --url postgres://localhost/app > docs/schema.mmd

# every format at once
remus --url postgres://localhost/app --format all --out-dir docs/schema

# credential-free: remus never sees your connection string
remus --print-sql | psql "$DATABASE_URL" -Atf - | remus --input - --format dbml

# conceptual view: junction tables collapse into N:N, no columns
remus --url ... --conceptual --no-attributes
```

`--url` falls back to `DATABASE_URL`. `--schema` restricts to named schemas.
`--views` adds views and materialized views to diagrams.

See **[examples/](examples/README.md)** for the four outputs side by side on two sample
schemas, with the diagrams rendered.

## Install

```bash
cargo install --git https://github.com/lupidae/remus remus
```

Requires Rust 1.97 or newer. TLS is built in (rustls with public roots), so
hosted providers with public certificates work without a system OpenSSL.

## How it works

One SQL query against `pg_catalog` returns the whole schema as a JSON document.
That query is embedded in the binary and printed verbatim by `--print-sql`, so
you can read exactly what remus looks at (never `information_schema`, never row
data) and run it yourself if handing a tool your credentials is not an option.
The JSON is the model; every other format is rendered from it.

## Formats and fidelity

JSON is authoritative. SQL covers everything the model carries. The two diagram
formats are lossy by nature; this table is the contract for what each keeps.

| Feature                          | JSON | SQL  | DBML                         | Mermaid                    |
|----------------------------------|:----:|:----:|------------------------------|----------------------------|
| Tables, columns, types           | ✓    | ✓    | ✓                            | ✓ (types as one token)     |
| Primary / unique / foreign keys  | ✓    | ✓    | ✓                            | ✓ (PK, FK, UK markers)     |
| Cardinality (crow's foot)        | ✓    | n/a  | ✓ (`>`, `-`, `<>`)           | ✓                          |
| FK actions, deferrable           | ✓    | ✓    | delete/update only           | ✗                          |
| Check and exclusion constraints  | ✓    | ✓    | table note                   | ✗                          |
| Enums                            | ✓    | ✓    | ✓ (`Enum` block)             | type name + "enum" note    |
| Domains                          | ✓    | ✓    | type name + base type note   | type name + base type note |
| Composite types                  | ✓    | ✓    | type name + note             | type name + note           |
| Identity / serial                | ✓    | ✓    | ✓ (`increment`)              | "identity" note            |
| Generated columns                | ✓    | ✓    | expression in note           | "generated" note           |
| Defaults                         | ✓    | ✓    | ✓                            | ✗                          |
| Indexes (expressions, partial)   | ✓    | ✓    | ✓ (predicate in note)        | ✗                          |
| Partitioning                     | ✓    | ✓    | parent only, key in note     | parent only                |
| Views, materialized views        | ✓    | ✓    | ✗ (listed in a comment)      | opt-in `--views`, as edges |
| Row level security, policies     | ✓    | ✓    | table note (count)           | ✗                          |
| Unlogged                         | ✓    | ✓    | table note                   | ✗                          |
| Comments                         | ✓    | ✓    | ✓ (notes)                    | column comments only       |
| Extensions                       | ✓    | ✓    | ✗                            | ✗                          |

### About the SQL output

It is **not** `pg_dump`. It emits DDL for the modelled objects only, in a
dependency-correct order, and its header says what is missing: functions,
triggers, grants, ownership, sequence state, foreign tables and roles. It is good
for spinning up a scratch copy of a schema and for reading a schema change as
SQL in a pull request. It is not a migration or a backup tool.

### Cardinality

Only what the catalog can prove. A foreign key targets a unique key, so a child
row has at most one parent, and exactly one when every FK column is `NOT NULL`.
A parent has at most one child only when the FK columns are themselves unique.
Nothing says a parent must have a child, so the child end is always "zero or".

### Conceptual mode

`--conceptual` hides tables that are pure associations, exactly two foreign keys
and no columns of their own beyond those and housekeeping (`id`, `created_at`,
...), and draws one N:N edge in their place. That is the only inference remus
makes, and it is deliberately conservative. Polymorphic associations
(`owner_type` / `owner_id`) are never guessed.

## Development

```bash
just check    # fmt (nightly rustfmt), check, clippy -D warnings, tests
just golden   # accept emitter output changes, then review the diff
just demo     # run against DATABASE_URL, everything under ./out
just examples # regenerate examples/*/out from each schema.sql
just site     # refresh the landing page's copy of the blog example
just site-deploy  # publish site/ to Cloudflare (wrangler.jsonc)
```

Emitter tests are golden files: `crates/remus-core/fixtures/showcase.json` is the
introspection of `showcase.sql`, a synthetic schema exercising every feature
above, and `crates/remus/tests/expected/` holds the rendered outputs. No database
is needed to run the tests.

Each format is its own crate — `remus-mermaid`, `remus-dbml`, `remus-sql` — over
the model in `remus-core`. They never depend on one another, so a consumer (a
wasm playground, say) can link a single format.

## Prior art

pgAdmin generates an ERD from a live database, and pgModeler is a mature
Postgres-native desktop modeller with reverse engineering and diff. Both are
better places to draw. remus is the Unix-shaped complement: text out, pipes in,
runs in CI, and produces the formats those editors and GitHub consume.
