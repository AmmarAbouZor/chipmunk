use std::path::PathBuf;

use clap::Subcommand;

#[derive(clap::Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Specify the path for the input file.
    #[arg(short, long = "input", required = true)]
    pub input: PathBuf,

    /// The database to use
    #[command(subcommand)]
    pub database: Database,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Database {
    /// Use SQLite.
    #[command(name = "sqlite")]
    Sqlite,
    /// Use DuckDB.
    #[command(name = "duckdb")]
    DuckDb,
}
