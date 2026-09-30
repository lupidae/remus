//! Golden-file tests: every `tests/fixtures/*.json` is rendered in every format
//! and variant and compared byte for byte with `tests/expected/`.
//!
//! After an intentional emitter change run `just golden` (`UPDATE_GOLDEN=1`) to
//! rewrite the expected files, then review the diff like any other code change.
//! No live database is involved; the fixtures are captured snapshots.

use std::{fs, path::Path};

use remus::format::Format;
use remus_core::{Schema, emit::DiagramOptions};

/// (file suffix, format, options). Diagram variants only exist for diagram formats.
fn variants() -> Vec<(String, Format, DiagramOptions)> {
    let mut variants = Vec::new();
    for format in Format::ALL {
        let emitter = format.emitter();
        let default = DiagramOptions::default();
        variants.push((
            emitter.extension().to_owned(),
            format.clone(),
            default.clone(),
        ));
        if !emitter.is_diagram() {
            continue;
        }
        variants.push((
            format!("conceptual.{}", emitter.extension()),
            format.clone(),
            DiagramOptions {
                conceptual: true,
                ..default.clone()
            },
        ));
        variants.push((
            format!("views.{}", emitter.extension()),
            format.clone(),
            DiagramOptions {
                views: true,
                ..default.clone()
            },
        ));
        variants.push((
            format!("boxes.{}", emitter.extension()),
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // The fixtures live with the query that produced them, in remus-core; only
    // the rendered expectations belong to this crate.
    let fixtures = root.join("../remus-core/fixtures");
    let expected_dir = root.join("tests/expected");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    let mut fixtures: Vec<_> = fs::read_dir(&fixtures)
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
            let actual = format.emitter().render(&schema, &options).unwrap();
            let expected_path = expected_dir.join(format!("{stem}.{suffix}"));
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
    // `str::lines` drops a trailing \r, so a file checked out with CRLF compares
    // equal line by line while differing byte for byte. Saying "<end of file>"
    // at that point sends the reader hunting for a diff that is not there.
    if expected.lines().eq(actual.lines()) {
        return format!(
            "{}: same lines, different bytes — the file on disk has {} line endings",
            path.display(),
            if expected.contains("\r\n") {
                "CRLF"
            } else {
                "unexpected"
            }
        );
    }
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

#[test]
fn a_line_ending_difference_says_so() {
    let message = describe_mismatch(Path::new("x.mmd"), "a\r\nb\r\n", "a\nb\n");
    assert!(message.contains("CRLF"), "{message}");
}

#[test]
fn a_real_difference_still_names_the_line() {
    let message = describe_mismatch(Path::new("x.mmd"), "a\nb\n", "a\nc\n");
    assert!(message.contains("line 2"), "{message}");
}
