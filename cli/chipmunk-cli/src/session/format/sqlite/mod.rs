use std::{fmt::Write, path::Path};

use anyhow::Context;
use queries::SqlQueries;
use rusqlite::{params_from_iter, Connection};

use super::MessageWriter;
use crate::session::parser::ParserInfo;

mod queries;

/// The length of messages buffer used to cache the messages after formatting them before
/// inserting them in the database as chunks.
const MESSAGES_CACHE_LEN: usize = 10000;

/// Structure to write parsed message in into SQLite database file.
#[derive(Debug)]
pub struct MsgSqliteWriter {
    /// Database connection.
    connection: Connection,
    /// Information of the parser used to parse the messages.
    parser_info: ParserInfo,
    /// The separator used for message columns in the parser used in indexer crates originally.
    indexer_cols_sep: char,
    /// SQL queries used withing the writer.
    sql_queries: SqlQueries,
    /// A vector of buffers to cache the formatted messages before inserting them into the database
    /// in bulks.
    messages_cache: Vec<Option<String>>,
    /// The next index to use in messages cache.
    cache_next_idx: usize,
    //TODO AAZ: For now I'm ignoring the separator for arguments inside payload.
}

impl MsgSqliteWriter {
    /// Creates a new instance with the given arguments.
    ///
    /// * `output_file`: The path for the output database file to write the message to.
    /// * `parser_info`: Infos of parser used to parse the data.
    /// * `indexer_args_sep`: Separator used for message payload arguments in the parser used
    ///   in indexer crates originally
    pub async fn new(
        output_file: &Path,
        parser_info: ParserInfo,
        indexer_cols_sep: char,
    ) -> anyhow::Result<Self> {
        let db_exists = output_file.exists();
        let connection =
            Connection::open(output_file).context("Error while connecting to database")?;
        let sql_queries = SqlQueries::new(&parser_info);
        if db_exists {
            //TODO AAZ: Validation?
        } else {
            connection
                .execute(&sql_queries.create_msg_table, [])
                .context("Error while defining tables in created database")?;
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
        })
    }

    /// Writes the messages in the cache to the database by using a transaction
    /// to write them in bulks at once.
    fn write_to_db(&mut self) -> anyhow::Result<()> {
        assert!(self.cache_next_idx <= self.messages_cache.len());

        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)?;

        let mut stm = tx.prepare_cached(&self.sql_queries.insert_msg)?;

        let cols_len = self.parser_info.columns.len();

        for idx in 0..self.cache_next_idx {
            // Fix broken messages by adding missing or ignoring additional columns.
            let msg_cols = self.messages_cache[idx]
                .as_ref()
                .expect("All messages below cache_next_idx must be initialized")
                .split(self.indexer_cols_sep)
                .chain(std::iter::repeat(""))
                .take(cols_len);

            stm.execute(params_from_iter(msg_cols))?;
        }

        drop(stm);

        tx.commit()?;

        self.cache_next_idx = 0;

        Ok(())
    }
}

impl MessageWriter for MsgSqliteWriter {
    fn write_msg<M>(&mut self, msg: &M) -> anyhow::Result<()>
    where
        M: parsers::LogMessage,
    {
        let msg_slot =
            &mut self.messages_cache[self.cache_next_idx].get_or_insert_with(String::new);

        msg_slot.clear();

        let _ = write!(msg_slot, "{msg}");

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

impl Drop for MsgSqliteWriter {
    fn drop(&mut self) {
        if self.cache_next_idx > 0 {
            if let Err(err) = self.write_to_db() {
                //TODO: Error should be logged and not printed to stderr.
                eprintln!("Error while writing messages to database. {err}");
            }
        }
    }
}
