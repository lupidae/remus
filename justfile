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
    UPDATE_GOLDEN=1 cargo test -p remus --test golden

# run the CLI against DATABASE_URL and write every format under ./out
demo *ARGS:
    cargo run -q -p remus -- --format all --out-dir out {{ARGS}}

# rebuild crates/remus-core/fixtures/showcase.json from showcase.sql in a scratch database on a
# local Postgres (PGURL: superuser connection to the `postgres` database). template0
# keeps extensions installed in the server's template1 out of the fixture; the
# timestamp is dropped so a regenerated fixture only differs when the schema does.
fixture PGURL="postgres://postgres:postgres@127.0.0.1:5432/postgres":
    #!/usr/bin/env bash
    set -euo pipefail
    admin="{{PGURL}}"
    scratch="{{trim_end_match(PGURL, "/postgres")}}/remus_showcase"
    psql "$admin" -v ON_ERROR_STOP=1 -qc "DROP DATABASE IF EXISTS remus_showcase WITH (FORCE)" -c "CREATE DATABASE remus_showcase TEMPLATE template0"
    psql "$scratch" -v ON_ERROR_STOP=1 -q -f crates/remus-core/fixtures/showcase.sql
    psql "$scratch" -Atf crates/remus-core/queries/introspect.sql \
        | python3 -c 'import sys, json; d = json.load(sys.stdin); d.pop("generated_at"); json.dump(d, open("crates/remus-core/fixtures/showcase.json", "w"), indent=2)'
    psql "$admin" -qc "DROP DATABASE remus_showcase WITH (FORCE)"
    just golden

# regenerate examples/*/ outputs from each example's schema.sql (same PGURL contract
# as `fixture`). Outputs are committed so the examples page renders on GitHub.
examples PGURL="postgres://postgres:postgres@127.0.0.1:5432/postgres":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build -q -p remus
    for dir in examples/*/; do
        name="remus_example_$(basename "$dir")"
        scratch="{{trim_end_match(PGURL, "/postgres")}}/$name"
        psql "{{PGURL}}" -v ON_ERROR_STOP=1 -qc "DROP DATABASE IF EXISTS $name WITH (FORCE)" -c "CREATE DATABASE $name TEMPLATE template0"
        psql "$scratch" -v ON_ERROR_STOP=1 -q -f "$dir/schema.sql"
        target/debug/remus -u "$scratch" -f all --out-dir "$dir/out"
        target/debug/remus -u "$scratch" -f mermaid --conceptual > "$dir/out/schema.conceptual.mmd"
        target/debug/remus -u "$scratch" -f mermaid --views > "$dir/out/schema.views.mmd"
        target/debug/remus -u "$scratch" -f mermaid --conceptual --no-attributes > "$dir/out/schema.boxes.mmd"
        psql "{{PGURL}}" -qc "DROP DATABASE $name WITH (FORCE)"
    done

# copy the blog example into the static site so the landing page shows real output
site: examples
    rm -rf site/examples && mkdir -p site/examples/blog/out
    cp examples/blog/schema.sql site/examples/blog/
    cp examples/blog/out/schema.{mmd,conceptual.mmd,views.mmd,dbml,sql,json} site/examples/blog/out/

# serve the landing page locally
site-serve:
    python3 -m http.server 8787 --directory site

# publish the landing page to Cloudflare (wrangler.jsonc; needs `npx wrangler login` once)
site-deploy: site
    npx --yes wrangler deploy
