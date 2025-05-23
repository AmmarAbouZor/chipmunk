use anyhow::ensure;
use clap::Parser;
use cli::Cli;
use paging::Direction;
use sqlite::SqliteDb;

mod cli;
mod paging;
mod sqlite;

pub fn run_app() -> anyhow::Result<()> {
    let cli = Cli::parse();

    ensure!(cli.input.exists(), "Input file doesn't exist");

    match cli.database {
        cli::Database::Sqlite => {
            let db = SqliteDb::create(&cli.input).unwrap();
            paging::run_benches(db, Direction::Backwards);
        }
        cli::Database::DuckDb => todo!("Duckdb isn't implemented yet"),
    };

    Ok(())
}
