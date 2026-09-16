default:
    @just --list

# rustfmt needs nightly for `imports_granularity` / `group_imports` (see rustfmt.toml)
fmt:
    cargo +nightly fmt

# the full gate: format, compile every target, lint hard, test
check: fmt
    cargo check --all-targets
    cargo clippy --all-targets -- -D warnings
    cargo test

# regenerate the golden files after an intentional emitter change, then review the diff
golden:
    UPDATE_GOLDEN=1 cargo test -p remus-core --test golden

# run the CLI against DATABASE_URL and write every format under ./out
demo *ARGS:
    cargo run -q -p remus -- --format all --out-dir out {{ARGS}}

# rebuild tests/fixtures/showcase.json from showcase.sql in a scratch database on a
# local Postgres (PGURL: superuser connection to the `postgres` database). The
# timestamp is dropped so a regenerated fixture only differs when the schema does.
fixture PGURL="postgres://postgres:postgres@127.0.0.1:5432/postgres":
    #!/usr/bin/env bash
    set -euo pipefail
    admin="{{PGURL}}"
    scratch="{{trim_end_match(PGURL, "/postgres")}}/remus_showcase"
    psql "$admin" -v ON_ERROR_STOP=1 -qc "DROP DATABASE IF EXISTS remus_showcase WITH (FORCE)" -c "CREATE DATABASE remus_showcase"
    psql "$scratch" -v ON_ERROR_STOP=1 -q -f crates/remus-core/tests/fixtures/showcase.sql
    psql "$scratch" -Atf crates/remus-core/queries/introspect.sql \
        | python3 -c 'import sys, json; d = json.load(sys.stdin); d.pop("generated_at"); json.dump(d, open("crates/remus-core/tests/fixtures/showcase.json", "w"), indent=2)'
    psql "$admin" -qc "DROP DATABASE remus_showcase WITH (FORCE)"
    just golden
