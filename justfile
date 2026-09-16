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
