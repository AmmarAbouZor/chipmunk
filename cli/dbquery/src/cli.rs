use std::path::PathBuf;

use clap::Subcommand;

#[derive(clap::Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Specify the path for the input file.
    #[arg(short, long = "input", required = true)]
    pub input: PathBuf,

    /// The database to use
    #[arg(short, long, required = true)]
    pub database: Database,

    #[command(subcommand)]
    pub function: Function,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum Database {
    /// Use SQLite.
    #[value(name = "sqlite")]
    Sqlite,
    /// Use DuckDB.
    #[value(name = "duckdb")]
    DuckDb,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Function {
    /// Run paging tests & benchmarks
    Paging {
        /// Scroll backwards
        #[arg(short, long, default_value_t = false)]
        backwards: bool,
    },
    /// Run search tests & benchmarks
    Search {
        /// Pattern to search for
        #[arg(index = 1)]
        pattern: String,
        /// Perform case sensitive search
        #[arg(short, long, default_value_t = false)]
        sensitive: bool,
        /// Perform Regex search
        #[arg(short, long, default_value_t = false)]
        regex: bool,
    },
}
