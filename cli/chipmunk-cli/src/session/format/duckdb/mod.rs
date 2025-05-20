use std::{fmt::Write, path::Path};

use crate::session::parser::ParserInfo;
use anyhow::Context;
use duckdb::{appender_params_from_iter, Connection};
use queries::{SqlQueries, MESSAGES_TABLE_NAME};

mod queries;

/// The length of messages buffer used to cache the messages after formatting them before
/// inserting them in the database as chunks.
const MESSAGES_CACHE_LEN: usize = 15000;

use super::MessageWriter;

/// Structure to write parsed message in into DuckDB database file.
#[derive(Debug)]
pub struct MsgDuckDbWriter {
    /// Database connection.
    connection: Connection,
    /// Information of the parser used to parse the messages.
    parser_info: ParserInfo,
    /// The separator used for message columns in the parser used in indexer crates originally.
    indexer_cols_sep: char,
    /// SQL queries used withing the writer.
    //TODO AAZ: Remove if not used.
    #[allow(unused)]
    sql_queries: SqlQueries,
    /// A vector of buffers to cache the formatted messages before inserting them into the database
    /// in bulks.
    messages_cache: Vec<Option<String>>,
    /// The next index to use in messages cache.
    cache_next_idx: usize,
    /// The current row index in the database
    /// NOTE: Append in DuckDB still doesn't support auto increment and we must handle it
    /// manually
    row_idx: usize,
    //TODO AAZ: For now I'm ignoring the separator for arguments inside payload.
}

impl MsgDuckDbWriter {
    /// Creates a new instance with the given arguments.
    ///
    /// * `output_file`: The path for the output database file to write the message to.
    /// * `parser_info`: Infos of parser used to parse the data.
    /// * `indexer_args_sep`: Separator used for message payload arguments in the parser used
    ///   in indexer crates originally
    pub fn new(
        output_file: &Path,
        parser_info: ParserInfo,
        indexer_cols_sep: char,
    ) -> anyhow::Result<Self> {
        let db_exists = output_file.exists();
        let connection =
            Connection::open(output_file).context("Error while connecting to database")?;
        let sql_queries = SqlQueries::new(&parser_info);
        let row_idx = if db_exists {
            let last_idx = connection
                .query_row(&sql_queries.last_msg_idx, [], |row| row.get::<_, usize>(0))
                .context("Error while retrieving the last index form messages database.")?;
            last_idx + 1
            //TODO: More validation in the final solution.
        } else {
            connection
                .execute(&sql_queries.create_msg_table, [])
                .context("Error while defining tables in created database")?;
            0
        };

        //NOTE: We may consider avoiding allocating the whole strings without having to use them.
        //One option is to have the cache as Vec<Option<String>>
        let messages_cache = vec![None; MESSAGES_CACHE_LEN];

        Ok(Self {
            connection,
            parser_info,
            sql_queries,
            indexer_cols_sep,
            messages_cache,
            cache_next_idx: 0,
            row_idx,
        })
    }

    /// Writes the messages in the cache to the database by using an appender
    /// to write them in bulks at once.
    fn write_to_db(&mut self) -> anyhow::Result<()> {
        assert!(self.cache_next_idx <= self.messages_cache.len());

        // let tx = self.connection.transaction()?;
        let mut app = self
            .connection
            .appender(MESSAGES_TABLE_NAME)
            .context("Error while creating appender")?;

        let iter = (0..self.cache_next_idx).map(|idx| {
            let mut msg_cols = Vec::with_capacity(self.parser_info.columns.len() + 1);

            msg_cols.extend(
                self.messages_cache[idx]
                    .as_ref()
                    .expect("All messages below cache_next_idx must be initialized")
                    .split(self.indexer_cols_sep),
            );

            while msg_cols.len() < self.parser_info.columns.len() + 1 {
                msg_cols.push("");
            }
            appender_params_from_iter(msg_cols)
        });

        app.append_rows(iter)?;

        app.flush()
            .context("Error while writing records to database via appender")?;

        self.cache_next_idx = 0;

        Ok(())
    }
}

impl MessageWriter for MsgDuckDbWriter {
    fn write_msg<M>(&mut self, msg: &M) -> anyhow::Result<()>
    where
        M: parsers::LogMessage,
    {
        let msg_slot =
            &mut self.messages_cache[self.cache_next_idx].get_or_insert_with(String::new);

        msg_slot.clear();

        // HACK: Add the index to the message with the same columns separator so it
        // will be included when inserted to the database.
        let _ = write!(msg_slot, "{}{}{msg}", self.row_idx, self.indexer_cols_sep);

        self.row_idx += 1;
        self.cache_next_idx += 1;

        if self.cache_next_idx < self.messages_cache.len() {
            Ok(())
        } else {
            self.write_to_db()
        }
    }

    async fn flush(&mut self) -> anyhow::Result<()> {
        if self.cache_next_idx > 0 {
            self.write_to_db()?;
        }
        Ok(())
    }
}

impl Drop for MsgDuckDbWriter {
    fn drop(&mut self) {
        if self.cache_next_idx > 0 {
            if let Err(err) = self.write_to_db() {
                //TODO: Error should be logged and not printed to stderr.
                eprintln!("Error while writing messages to database. {err}");
            }
        }
    }
}
