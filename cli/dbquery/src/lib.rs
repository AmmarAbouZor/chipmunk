use anyhow::ensure;
use clap::Parser;
use cli::{Cli, Function};
use duck::DuckDb;
use paging::Direction;
use search::{Case, SearchType};
use sqlite::SqliteDb;

mod cli;
mod duck;
mod paging;
mod search;
mod sqlite;

pub fn run_app() -> anyhow::Result<()> {
    let cli = Cli::parse();

    ensure!(cli.input.exists(), "Input file doesn't exist");

    match cli.function {
        Function::Paging { backwards } => {
            let dir = if backwards {
                Direction::Backwards
            } else {
                Direction::Forwards
            };

            match cli.database {
                cli::Database::Sqlite => {
                    let db = SqliteDb::create(&cli.input).unwrap();
                    paging::run_benches(db, dir);
                }
                cli::Database::DuckDb => {
                    let db = DuckDb::create(&cli.input).unwrap();
                    paging::run_benches(db, dir);
                }
            };
        }

        Function::Search {
            pattern,
            sensitive,
            regex,
        } => {
            let search_type = if regex {
                SearchType::Regex
            } else {
                SearchType::Like
            };

            let case = if sensitive {
                Case::Sensitive
            } else {
                Case::Insensitive
            };
            match cli.database {
                cli::Database::Sqlite => {
                    let db = SqliteDb::create(&cli.input).unwrap();
                    search::run_benches(db, pattern, search_type, case);
                }
                cli::Database::DuckDb => {
                    let db = DuckDb::create(&cli.input).unwrap();
                    search::run_benches(db, pattern, search_type, case);
                }
            };
        }
    }

    Ok(())
}
