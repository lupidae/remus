//! The guided flow: `remus` with nothing to go on asks a few questions instead
//! of printing usage.
//!
//! Every answer maps to a flag, and the last line of the run prints the command
//! that would have skipped the questions, so the guided path teaches the
//! scriptable one rather than replacing it.

use std::{
    error::Error as _,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use inquire::{InquireError, MultiSelect, Text, list_option::ListOption};
use owo_colors::OwoColorize;
use remus_core::{Schema, emit::DiagramOptions};

use crate::{
    Error,
    cli::{self, Cli, Destination, Written},
    format::Format,
    introspect,
};

pub async fn run(cli: &Cli) -> Result<(), Error> {
    banner();

    let (source, mut schema) = load(cli).await?;
    let available = schema_names(&schema);
    let schemas = ask_schemas(&available)?;
    schema.retain_schemas(&schemas);

    let formats = ask_formats(cli)?;
    let options = ask_diagram_options(cli, &formats)?;
    let directory = ask_directory(cli)?;

    let written = cli::emit(
        &schema,
        &formats,
        &options,
        &Destination::Directory(directory.clone()),
    )?;
    report(&written);
    replay(
        &source, &schemas, &available, &formats, &options, &directory,
    );
    Ok(())
}

fn banner() {
    eprintln!();
    eprintln!(
        "  {}  {}",
        "remus".bold(),
        "your Postgres schema, as text".dimmed()
    );
    eprintln!();
}

/// Keeps asking until something answers: a typo in a URL is the one mistake
/// every first run makes, and re-running the whole command to fix it is what
/// this flow exists to avoid.
///
/// The answer is a connection string or, for anyone who would rather not hand
/// over credentials, the path to a payload captured with `--print-sql`.
async fn load(cli: &Cli) -> Result<(String, Schema), Error> {
    let mut suggestion = cli.suggested_url();
    loop {
        let answer = Text::new("Database")
            .with_default(&suggestion)
            .with_help_message("connection string, or a file captured with --print-sql")
            .prompt()
            .map_err(prompt_error)?;

        let found = if answer.starts_with("postgres://") || answer.starts_with("postgresql://") {
            eprint!("  {} connecting…", "↳".dimmed());
            let _ = std::io::stderr().flush();
            introspect::fetch(&answer).await
        } else {
            fs::read_to_string(&answer)
                .map_err(|source| Error::Read {
                    path: PathBuf::from(&answer),
                    source,
                })
                .and_then(|json| Ok(Schema::from_json(&json)?))
        };

        match found {
            Ok(schema) => {
                eprintln!("\r{CLEAR}  {} {}", "✓".green(), census(&schema));
                if schema.entities.is_empty() {
                    eprintln!(
                        "  {}",
                        "nothing visible here — check the role's privileges".dimmed()
                    );
                }
                eprintln!();
                return Ok((answer, schema));
            }
            Err(err) => {
                eprintln!("\r{CLEAR}  {} {}", "✗".red(), err);
                let mut cause = err.source();
                while let Some(inner) = cause {
                    eprintln!("    {}", inner.to_string().dimmed());
                    cause = inner.source();
                }
                suggestion = answer;
            }
        }
    }
}

/// Erase the "connecting…" line before the verdict replaces it.
const CLEAR: &str = "\x1b[2K";

fn census(schema: &Schema) -> String {
    let tables = schema.entities.iter().filter(|e| e.kind.is_table()).count();
    let views = schema.entities.iter().filter(|e| e.kind.is_view()).count();
    let mut census = format!("{} · {} tables", schema.database, tables);
    if views > 0 {
        census.push_str(&format!(", {views} views"));
    }
    census
}

fn schema_names(schema: &Schema) -> Vec<String> {
    let mut names: Vec<String> = schema.entities.iter().map(|e| e.schema.clone()).collect();
    names.sort_unstable();
    names.dedup();
    names
}

fn ask_schemas(available: &[String]) -> Result<Vec<String>, Error> {
    // Nothing to choose from is not a question worth asking.
    if available.len() < 2 {
        return Ok(available.to_vec());
    }
    let every: Vec<usize> = (0..available.len()).collect();
    pick("Schemas", available.to_vec(), &every, ToString::to_string)
}

fn ask_formats(cli: &Cli) -> Result<Vec<Format>, Error> {
    // Nothing asked for means everything: one introspection is the cost either
    // way, and four files beat coming back for the one that was missed.
    let chosen = cli
        .explicit_formats()
        .unwrap_or_else(|| Format::ALL.to_vec());
    let menu: Vec<Choice> = Format::MENU.iter().cloned().map(Choice).collect();
    let preselected: Vec<usize> = menu
        .iter()
        .enumerate()
        .filter(|(_, choice)| chosen.contains(&choice.0))
        .map(|(index, _)| index)
        .collect();

    let picked = pick("Formats", menu, &preselected, |choice| choice.0.to_string())?;
    // Written in the order --out-dir uses, whatever order they were ticked in.
    Ok(Format::ALL
        .into_iter()
        .filter(|format| picked.iter().any(|choice| &choice.0 == format))
        .collect())
}

fn ask_diagram_options(cli: &Cli, formats: &[Format]) -> Result<DiagramOptions, Error> {
    let mut options = cli.diagram_options();
    if !formats.iter().any(|f| f.emitter().is_diagram()) {
        return Ok(options);
    }

    let toggles = vec![
        "collapse junction tables into many-to-many",
        "drop the columns, keep boxes and lines",
        "include views and materialized views",
    ];
    let preselected: Vec<usize> = [options.conceptual, !options.attributes, options.views]
        .iter()
        .enumerate()
        .filter(|(_, on)| **on)
        .map(|(index, _)| index)
        .collect();

    let summarise = |picked: &[ListOption<&&str>]| match picked.len() {
        0 => "physical".to_string(),
        _ => picked
            .iter()
            .map(|option| option.value.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    };
    let picked = MultiSelect::new("Diagram", toggles.clone())
        .with_default(&preselected)
        .with_formatter(&summarise)
        .with_help_message("space toggles; none of them is the physical schema")
        .prompt()
        .map_err(prompt_error)?;

    options.conceptual = picked.contains(&toggles[0]);
    options.attributes = !picked.contains(&toggles[1]);
    options.views = picked.contains(&toggles[2]);
    Ok(options)
}

fn ask_directory(cli: &Cli) -> Result<PathBuf, Error> {
    let default = cli
        .out_dir()
        .map(|dir| dir.display().to_string())
        .unwrap_or_else(|| "schema".to_string());
    Text::new("Folder")
        .with_default(&default)
        .with_help_message("one schema.<ext> per format goes here")
        .prompt()
        .map(PathBuf::from)
        .map_err(prompt_error)
}

/// A multi-select that will not take "nothing" for an answer. `summary` keeps
/// the answered line short when the list itself carries explanatory text.
fn pick<T: Clone + std::fmt::Display>(
    message: &str,
    options: Vec<T>,
    preselected: &[usize],
    summary: impl Fn(&T) -> String,
) -> Result<Vec<T>, Error> {
    let formatter = |picked: &[ListOption<&T>]| {
        picked
            .iter()
            .map(|option| summary(option.value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    loop {
        // MultiSelect consumes its options, so rebuild the prompt each round.
        let picked = MultiSelect::new(message, options.clone())
            .with_default(preselected)
            .with_formatter(&formatter)
            .prompt();
        match picked {
            Ok(picked) if picked.is_empty() => {
                eprintln!("  {}", "pick at least one with space".dimmed());
            }
            Ok(picked) => return Ok(picked),
            Err(err) => return Err(prompt_error(err)),
        }
    }
}

fn report(written: &[Written]) {
    eprintln!();
    for file in written {
        let path = file
            .path
            .as_ref()
            .map_or_else(String::new, |p| p.display().to_string());
        eprintln!(
            "  {} {:<24} {}",
            "✓".green(),
            path,
            bytes(file.bytes).dimmed()
        );
    }
}

/// The same run as a single command, so the next one needs no questions.
fn replay(
    source: &str,
    schemas: &[String],
    available: &[String],
    formats: &[Format],
    options: &DiagramOptions,
    directory: &Path,
) {
    eprintln!();
    eprintln!("  {}", "next time, in one line:".dimmed());
    eprintln!(
        "  {}",
        command(source, schemas, available, formats, options, directory).cyan()
    );
    eprintln!();
}

fn command(
    source: &str,
    schemas: &[String],
    available: &[String],
    formats: &[Format],
    options: &DiagramOptions,
    directory: &Path,
) -> String {
    // Never echo a password back: the URL came from, or belongs in, the env.
    let mut command = if source.starts_with("postgres") {
        String::from("remus -u \"$DATABASE_URL\"")
    } else {
        format!("remus -i {source}")
    };
    // `-s` only earns its place when something was left out.
    if available.iter().any(|name| !schemas.contains(name)) {
        for name in schemas {
            command.push_str(&format!(" -s {name}"));
        }
    }
    let names: Vec<String> = formats.iter().map(Format::to_string).collect();
    command.push_str(&format!(" -f {}", names.join(",")));
    if options.conceptual {
        command.push_str(" --conceptual");
    }
    if !options.attributes {
        command.push_str(" --no-attributes");
    }
    if options.views {
        command.push_str(" --views");
    }
    command.push_str(&format!(" --out-dir {}", directory.display()));
    command
}

fn bytes(count: usize) -> String {
    if count < 1024 {
        format!("{count} B")
    } else {
        format!("{:.1} kB", count as f64 / 1024.0)
    }
}

fn prompt_error(err: InquireError) -> Error {
    match err {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => Error::Canceled,
        other => Error::Prompt(other),
    }
}

/// A format with its one-line purpose, for the menu only.
#[derive(Clone)]
struct Choice(Format);

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<8} {}", self.0.to_string(), self.0.purpose().dimmed())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use remus_core::emit::DiagramOptions;

    use super::{bytes, command};
    use crate::format::Format;

    fn replay(schemas: &[&str], available: &[&str], options: DiagramOptions) -> String {
        command(
            "postgres://localhost/app",
            &schemas.iter().map(ToString::to_string).collect::<Vec<_>>(),
            &available
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            &[Format::Mermaid],
            &options,
            Path::new("schema"),
        )
    }

    #[test]
    fn sizes_read_as_sizes() {
        assert_eq!(bytes(12), "12 B");
        assert_eq!(bytes(2048), "2.0 kB");
    }

    #[test]
    fn the_replay_never_echoes_the_connection_string() {
        let replayed = replay(&["public"], &["public"], DiagramOptions::default());
        assert!(replayed.starts_with("remus -u \"$DATABASE_URL\""));
        assert!(!replayed.contains("localhost"));
    }

    #[test]
    fn a_schema_filter_is_only_replayed_when_it_filters() {
        assert!(!replay(&["public"], &["public"], DiagramOptions::default()).contains("-s"));
        assert!(
            replay(&["shop"], &["audit", "shop"], DiagramOptions::default()).contains("-s shop")
        );
    }

    #[test]
    fn diagram_answers_come_back_as_flags() {
        let replayed = replay(
            &["public"],
            &["public"],
            DiagramOptions {
                conceptual: true,
                attributes: false,
                views: true,
            },
        );
        assert!(replayed.contains("--conceptual"));
        assert!(replayed.contains("--no-attributes"));
        assert!(replayed.contains("--views"));
    }
}
