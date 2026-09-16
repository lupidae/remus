use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, ValueEnum};
use remus_core::{
    INTROSPECT_SQL, Schema,
    emit::{self, DiagramOptions, Format},
};

use crate::{Error, introspect};

#[derive(Debug, Parser)]
#[command(name = "remus", version)]
#[command(about = "Export a PostgreSQL schema as JSON, Mermaid, DBML or SQL DDL.")]
#[command(after_help = "\
Examples:
  remus -u postgres://localhost/app > schema.mmd
  remus -u postgres://localhost/app -f all --out-dir docs/schema
  remus --print-sql | psql \"$DATABASE_URL\" -Atf - | remus --input - -f dbml
  remus -u ... --conceptual --no-attributes        # boxes and lines, junctions collapsed")]
pub struct Cli {
    /// Connection string. Falls back to DATABASE_URL.
    #[arg(
        short,
        long,
        env = "DATABASE_URL",
        value_name = "URL",
        hide_env_values = true
    )]
    url: Option<String>,

    /// Read a captured introspection payload instead of connecting; `-` is stdin.
    /// Produce one with `--print-sql`, so remus never needs your credentials.
    #[arg(short, long, value_name = "FILE")]
    input: Option<PathBuf>,

    /// Print the introspection SQL and exit. It reads pg_catalog only.
    #[arg(long)]
    print_sql: bool,

    /// Output format. Repeat or comma-separate for several; `all` for every one.
    /// More than one format needs --out-dir.
    #[arg(
        short,
        long,
        value_enum,
        value_name = "FORMAT",
        value_delimiter = ',',
        default_value = "mermaid"
    )]
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
    fn formats(&self) -> Vec<Format> {
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
        formats
    }

    fn diagram_options(&self) -> DiagramOptions {
        DiagramOptions {
            conceptual: self.conceptual,
            attributes: !self.no_attributes,
            views: self.views,
        }
    }
}

enum Destination {
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

    fn write(&self, format: &Format, rendered: &str) -> Result<(), Error> {
        match self {
            Self::Stdout => {
                let mut stdout = io::stdout().lock();
                stdout
                    .write_all(rendered.as_bytes())
                    .and_then(|()| stdout.flush())
                    .map_err(|source| Error::Write {
                        path: PathBuf::from("<stdout>"),
                        source,
                    })
            }
            Self::File(path) => write_file(path, rendered),
            Self::Directory(dir) => {
                fs::create_dir_all(dir).map_err(|source| Error::Write {
                    path: dir.clone(),
                    source,
                })?;
                write_file(
                    &dir.join(format!("schema.{}", format.extension())),
                    rendered,
                )
            }
        }
    }
}

fn write_file(path: &Path, rendered: &str) -> Result<(), Error> {
    fs::write(path, rendered).map_err(|source| Error::Write {
        path: path.to_path_buf(),
        source,
    })
}

pub async fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    if cli.print_sql {
        print!("{INTROSPECT_SQL}");
        return Ok(());
    }

    let formats = cli.formats();
    let destination = Destination::resolve(&cli, formats.len())?;

    // Introspect once, render N times: the payload is the expensive part.
    let mut schema = load_schema(&cli).await?;
    schema.retain_schemas(&cli.schemas);
    if schema.entities.is_empty() {
        eprintln!("remus: no entities found; check --schema filters and connection privileges");
    }

    let options = cli.diagram_options();
    for format in &formats {
        let rendered = emit::render(&schema, format, &options)?;
        destination.write(format, &rendered)?;
    }
    Ok(())
}

/// `--input` wins over `--url` so an exported DATABASE_URL in the environment
/// never silently overrides an explicit file.
async fn load_schema(cli: &Cli) -> Result<Schema, Error> {
    match (&cli.input, &cli.url) {
        (Some(path), _) => Ok(Schema::from_json(&read_input(path)?)?),
        (None, Some(url)) => introspect::fetch(url).await,
        (None, None) => Err(Error::NoSource),
    }
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
    use remus_core::emit::Format;

    use super::{Cli, Destination};

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
}
