//! Golden-file tests: every `tests/fixtures/*.json` is rendered in every format
//! and variant and compared byte for byte with `tests/expected/`.
//!
//! After an intentional emitter change run `just golden` (`UPDATE_GOLDEN=1`) to
//! rewrite the expected files, then review the diff like any other code change.
//! No live database is involved; the fixtures are captured snapshots.

use std::{fs, path::Path};

use remus_core::{
    Schema,
    emit::{self, DiagramOptions, Format},
};

/// (file suffix, format, options). Diagram variants only exist for diagram formats.
fn variants() -> Vec<(String, Format, DiagramOptions)> {
    let mut variants = Vec::new();
    for format in Format::ALL {
        let default = DiagramOptions::default();
        variants.push((
            format.extension().to_owned(),
            format.clone(),
            default.clone(),
        ));
        if !format.is_diagram() {
            continue;
        }
        variants.push((
            format!("conceptual.{}", format.extension()),
            format.clone(),
            DiagramOptions {
                conceptual: true,
                ..default.clone()
            },
        ));
        variants.push((
            format!("views.{}", format.extension()),
            format.clone(),
            DiagramOptions {
                views: true,
                ..default.clone()
            },
        ));
        variants.push((
            format!("boxes.{}", format.extension()),
            format,
            DiagramOptions {
                attributes: false,
                ..default
            },
        ));
    }
    variants
}

#[test]
fn every_fixture_matches_its_expected_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    let mut fixtures: Vec<_> = fs::read_dir(root.join("fixtures"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    fixtures.sort();
    assert!(!fixtures.is_empty(), "no fixtures found");

    for fixture in fixtures {
        let stem = fixture.file_stem().unwrap().to_str().unwrap();
        let schema = Schema::from_json(&fs::read_to_string(&fixture).unwrap())
            .unwrap_or_else(|err| panic!("{stem}: {err}"));
        for (suffix, format, options) in variants() {
            let actual = emit::render(&schema, &format, &options).unwrap();
            let expected_path = root.join("expected").join(format!("{stem}.{suffix}"));
            if update {
                fs::write(&expected_path, &actual).unwrap();
                continue;
            }
            let expected = fs::read_to_string(&expected_path).unwrap_or_default();
            if actual != expected {
                failures.push(describe_mismatch(&expected_path, &expected, &actual));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} golden file(s) differ (run `just golden` to accept):\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn describe_mismatch(path: &Path, expected: &str, actual: &str) -> String {
    let first_diff = expected
        .lines()
        .zip(actual.lines())
        .position(|(e, a)| e != a)
        .unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
    let expected_line = expected.lines().nth(first_diff).unwrap_or("<end of file>");
    let actual_line = actual.lines().nth(first_diff).unwrap_or("<end of file>");
    format!(
        "{}: first difference at line {}\n  expected: {expected_line}\n  actual:   {actual_line}",
        path.display(),
        first_diff + 1
    )
}
