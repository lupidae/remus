use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, ValueEnum};
use remus_core::{INTROSPECT_SQL, Schema, emit::DiagramOptions};

use crate::{Error, format::Format, guide, introspect};

/// Connection string used when nothing else is known. Guessed, not required:
/// the guided flow shows it as an editable default.
pub const DEFAULT_URL: &str = "postgres://localhost/postgres";

#[derive(Debug, Parser)]
#[command(name = "remus", version)]
#[command(about = "Export a PostgreSQL schema as JSON, Mermaid, DBML or SQL DDL.")]
#[command(after_help = "\
Run `remus` with no arguments in a terminal and it asks four questions instead.

Examples:
  remus                                            guided: connect, pick, write
  remus -u postgres://localhost/app > schema.mmd
  remus -u postgres://localhost/app -f all --out-dir docs/schema
  remus --print-sql | psql \"$DATABASE_URL\" -Atf - | remus -i - -f dbml")]
pub struct Cli {
    /// Connection string. Falls back to DATABASE_URL.
    #[arg(short, long, value_name = "URL")]
    url: Option<String>,

    /// Read a captured introspection payload instead of connecting; `-` is stdin.
    /// Produce one with `--print-sql`, so remus never needs your credentials.
    #[arg(short, long, value_name = "FILE")]
    input: Option<PathBuf>,

    /// Print the introspection SQL and exit. It reads pg_catalog only.
    #[arg(long)]
    print_sql: bool,

    /// Output format: json, mermaid, dbml, sql, or all. Repeatable and
    /// comma-separated. [default: mermaid] More than one needs --out-dir.
    #[arg(short, long, value_enum, value_name = "FORMAT", value_delimiter = ',')]
    format: Vec<FormatArg>,

    /// Write to this file instead of stdout (single format only).
    #[arg(short, long, value_name = "FILE", conflicts_with = "out_dir")]
    out: Option<PathBuf>,

    /// Write schema.<ext> for every requested format into this directory.
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// Restrict to these schemas (repeatable). Default: every non-system schema.
    #[arg(short, long = "schema", value_name = "NAME")]
    schemas: Vec<String>,

    /// Collapse pure junction tables into N:N relationships. Diagram formats only;
    /// JSON and SQL always describe the physical schema.
    #[arg(short, long)]
    conceptual: bool,

    /// Include views and materialized views in diagrams.
    #[arg(long)]
    views: bool,

    /// Diagrams without columns: boxes and lines only.
    #[arg(long)]
    no_attributes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
enum FormatArg {
    Json,
    Mermaid,
    Dbml,
    Sql,
    All,
}

impl Cli {
    /// Requested formats, `all` expanded, duplicates dropped, order kept.
    pub fn formats(&self) -> Vec<Format> {
        let mut formats: Vec<Format> = Vec::new();
        for arg in &self.format {
            let expanded: Vec<Format> = match arg {
                FormatArg::Json => vec![Format::Json],
                FormatArg::Mermaid => vec![Format::Mermaid],
                FormatArg::Dbml => vec![Format::Dbml],
                FormatArg::Sql => vec![Format::Sql],
                FormatArg::All => Format::ALL.to_vec(),
            };
            for format in expanded {
                if !formats.contains(&format) {
                    formats.push(format);
                }
            }
        }
        if formats.is_empty() {
            formats.push(Format::Mermaid);
        }
        formats
    }

    pub fn diagram_options(&self) -> DiagramOptions {
        DiagramOptions {
            conceptual: self.conceptual,
            attributes: !self.no_attributes,
            views: self.views,
        }
    }

    /// The formats actually asked for, or `None` when the guided flow should
    /// choose for itself rather than inherit the implicit Mermaid default.
    pub fn explicit_formats(&self) -> Option<Vec<Format>> {
        (!self.format.is_empty()).then(|| self.formats())
    }

    pub fn schemas(&self) -> &[String] {
        &self.schemas
    }

    pub fn out_dir(&self) -> Option<&Path> {
        self.out_dir.as_deref()
    }

    /// The URL to offer first in the guided flow.
    pub fn suggested_url(&self) -> String {
        self.url
            .clone()
            .or_else(database_url)
            .unwrap_or_else(|| DEFAULT_URL.to_string())
    }

    /// Whether a source was named at all. `--input` wins over `--url` so an
    /// exported DATABASE_URL never silently overrides an explicit file.
    fn source(&self) -> Option<Source> {
        match (&self.input, &self.url) {
            (Some(path), _) => Some(Source::Input(path.clone())),
            (None, Some(url)) => Some(Source::Url(url.clone())),
            (None, None) => database_url().map(Source::Url),
        }
    }
}

enum Source {
    Url(String),
    Input(PathBuf),
}

fn database_url() -> Option<String> {
    env::var("DATABASE_URL").ok().filter(|url| !url.is_empty())
}

pub enum Destination {
    Stdout,
    File(PathBuf),
    Directory(PathBuf),
}

impl Destination {
    fn resolve(cli: &Cli, format_count: usize) -> Result<Self, Error> {
        match (&cli.out_dir, &cli.out) {
            (Some(dir), _) => Ok(Self::Directory(dir.clone())),
            (None, _) if format_count > 1 => Err(Error::NeedsOutDir {
                count: format_count,
            }),
            (None, Some(file)) => Ok(Self::File(file.clone())),
            (None, None) => Ok(Self::Stdout),
        }
    }

    /// The path written to, or `None` for stdout.
    fn write(&self, extension: &str, rendered: &str) -> Result<Option<PathBuf>, Error> {
        match self {
            Self::Stdout => {
                let mut stdout = io::stdout().lock();
                stdout
                    .write_all(rendered.as_bytes())
                    .and_then(|()| stdout.flush())
                    .map_err(|source| Error::Write {
                        path: PathBuf::from("<stdout>"),
                        source,
                    })?;
                Ok(None)
            }
            Self::File(path) => write_file(path, rendered).map(Some),
            Self::Directory(dir) => {
                fs::create_dir_all(dir).map_err(|source| Error::Write {
                    path: dir.clone(),
                    source,
                })?;
                write_file(&dir.join(format!("schema.{extension}")), rendered).map(Some)
            }
        }
    }
}

fn write_file(path: &Path, rendered: &str) -> Result<PathBuf, Error> {
    fs::write(path, rendered)
        .map(|()| path.to_path_buf())
        .map_err(|source| Error::Write {
            path: path.to_path_buf(),
            source,
        })
}

/// One rendered format, and where it landed.
pub struct Written {
    pub format: Format,
    pub path: Option<PathBuf>,
    pub bytes: usize,
}

/// Render once per format from a single introspection: the payload is the
/// expensive part, the emitters are pure.
pub fn emit(
    schema: &Schema,
    formats: &[Format],
    options: &DiagramOptions,
    destination: &Destination,
) -> Result<Vec<Written>, Error> {
    formats
        .iter()
        .map(|format| {
            let emitter = format.emitter();
            let rendered = emitter.render(schema, options)?;
            Ok(Written {
                format: format.clone(),
                path: destination.write(emitter.extension(), &rendered)?,
                bytes: rendered.len(),
            })
        })
        .collect()
}

pub async fn run() -> Result<(), Error> {
    let bare = env::args_os().len() == 1;
    let cli = Cli::parse();

    if cli.print_sql {
        print!("{INTROSPECT_SQL}");
        return Ok(());
    }

    // Guided when there is a human to guide: a bare `remus`, or flags that name
    // no database. Anything piped or redirected keeps the scriptable behaviour.
    let interactive = io::stdin().is_terminal() && io::stderr().is_terminal();
    let source = cli.source();
    if interactive && (bare || source.is_none()) {
        return guide::run(&cli).await;
    }

    let formats = cli.formats();
    let destination = Destination::resolve(&cli, formats.len())?;

    let mut schema = match source.ok_or(Error::NoSource)? {
        Source::Input(path) => Schema::from_json(&read_input(&path)?)?,
        Source::Url(url) => introspect::fetch(&url).await?,
    };
    schema.retain_schemas(&cli.schemas);
    if schema.entities.is_empty() {
        eprintln!("remus: no entities found; check --schema filters and connection privileges");
    }

    emit(&schema, &formats, &cli.diagram_options(), &destination)?;
    Ok(())
}

fn read_input(path: &Path) -> Result<String, Error> {
    let read = || {
        if path.as_os_str() == "-" {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer)
        } else {
            fs::read_to_string(path)
        }
    };
    read().map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Destination};
    use crate::format::Format;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("remus").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn default_format_is_mermaid_to_stdout() {
        let cli = parse(&[]);
        assert_eq!(cli.formats(), [Format::Mermaid]);
        assert!(matches!(
            Destination::resolve(&cli, 1),
            Ok(Destination::Stdout)
        ));
    }

    #[test]
    fn all_expands_in_canonical_order_without_duplicates() {
        let cli = parse(&["-f", "sql,all", "--format", "json"]);
        assert_eq!(
            cli.formats(),
            [Format::Sql, Format::Json, Format::Mermaid, Format::Dbml]
        );
    }

    #[test]
    fn several_formats_need_a_directory() {
        let cli = parse(&["-f", "json", "-f", "sql"]);
        assert!(Destination::resolve(&cli, 2).is_err());

        let cli = parse(&["-f", "all", "--out-dir", "out"]);
        assert!(matches!(
            Destination::resolve(&cli, 4),
            Ok(Destination::Directory(_))
        ));
    }

    #[test]
    fn out_and_out_dir_are_exclusive() {
        let parsed = Cli::try_parse_from(["remus", "-o", "a.mmd", "--out-dir", "out"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn an_explicit_url_is_the_suggestion() {
        assert_eq!(
            parse(&["-u", "postgres://host/db"]).suggested_url(),
            "postgres://host/db"
        );
    }
}
