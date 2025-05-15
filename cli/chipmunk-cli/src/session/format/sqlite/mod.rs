use std::{fmt::Write, path::Path};

use anyhow::{ensure, Context};
use queries::SqlQueries;
use rusqlite::{params_from_iter, Connection};

use super::MessageWriter;
use crate::session::parser::ParserInfo;

mod queries;

/// Structure to write parsed message in into SQLite database file.
#[derive(Debug)]
pub struct MsgSqliteWriter {
    /// Database connection.
    connection: Connection,
    /// Information of the parser used to parse the messages.
    parser_info: ParserInfo,
    sql_queries: SqlQueries,
    /// Parse the message text with its delimiters into this buffer to avoid
    /// allocating memory on each message.
    origin_msg_buffer: String,
    /// The separator used for message columns in the parser used in indexer crates originally.
    indexer_cols_sep: char,
    //TODO AAZ: For now I'm ignoring the separator for arguments inside payload.
}

impl MsgSqliteWriter {
    /// Creates a new instance with the given arguments.
    ///
    /// * `output_file`: The path for the output database file to write the message to.
    //TODO AAZ: Complete the docs.
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

        Ok(Self {
            connection,
            parser_info,
            sql_queries,
            origin_msg_buffer: String::new(),
            indexer_cols_sep,
        })
    }
}

impl MessageWriter for MsgSqliteWriter {
    fn write_msg<M>(&mut self, msg: &M) -> anyhow::Result<()>
    where
        M: parsers::LogMessage,
    {
        self.origin_msg_buffer.clear();

        let _ = write!(&mut self.origin_msg_buffer, "{msg}");
        let msg_cols: Vec<_> = self
            .origin_msg_buffer
            .split(self.indexer_cols_sep)
            .collect();

        ensure!(
            msg_cols.len() == self.parser_info.columns.len(),
            "Message columns don't match columns definitions for the provided format"
        );

        self.connection
            .execute(&self.sql_queries.insert_msg, params_from_iter(msg_cols))?;

        Ok(())
    }

    async fn flush(&mut self) -> anyhow::Result<()> {
        //TODO AAZ: For now flush isn't needed.
        // self.connection.flush_prepared_statement_cache();
        Ok(())
    }
}
