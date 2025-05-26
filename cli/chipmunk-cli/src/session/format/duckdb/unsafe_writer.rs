use std::path::Path;

use super::queries::{SqlQueries, MESSAGES_TABLE_NAME};
use crate::session::parser::ParserInfo;
use anyhow::Context;
use duckdb::{appender_params_from_iter, Appender, Connection};

use super::MessageWriter;

/// Structure to write parsed message in into DuckDB database file.
#[derive(Debug)]
pub struct MsgDuckDbWriter {
    // We need to keep connection alive because appender has reference to it.
    /// Database connection.
    _connection: Connection,
    /// Information of the parser used to parse the messages.
    parser_info: ParserInfo,
    /// The separator used for message columns in the parser used in indexer crates originally.
    indexer_cols_sep: char,
    /// SQL queries used withing the writer.
    //TODO AAZ: Remove if not used.
    #[allow(unused)]
    sql_queries: SqlQueries,
    /// The current row index in the database
    /// NOTE: Append in DuckDB still doesn't support auto increment and we must handle it
    /// manually
    row_idx: usize,
    /// Parse the message text with its delimiters into this buffer to avoid
    /// allocating memory on each message.
    msg_buffer: String,
    /// Appender to manage adding items to database in bulks.
    app: Appender<'static>,
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

        let app = connection
            .appender(MESSAGES_TABLE_NAME)
            .context("Error while creating appender")?;

        // SAFETY: Both Connection and Appender are fields on the same structs, so they will live
        // and get destroyed together.
        // This is a workaround to solve self-referencing fields in rust.
        let app = unsafe { std::mem::transmute::<Appender<'_>, Appender<'static>>(app) };

        Ok(Self {
            _connection: connection,
            msg_buffer: String::new(),
            parser_info,
            sql_queries,
            indexer_cols_sep,
            row_idx,
            app,
        })
    }

    /// Writes the messages in the cache to the database by using an appender
    /// to write them in bulks at once.
    fn write_to_db(&mut self) -> anyhow::Result<()> {
        self.app
            .flush()
            .context("Error while writing records to database via appender")?;

        Ok(())
    }
}

impl MessageWriter for MsgDuckDbWriter {
    fn write_msg<M>(&mut self, msg: &M) -> anyhow::Result<()>
    where
        M: parsers::LogMessage,
    {
        use std::fmt::Write;
        self.msg_buffer.clear();

        // HACK: Add the index to the message with the same columns separator so it
        // will be included when inserted to the database.
        let _ = write!(
            &mut self.msg_buffer,
            "{}{}{msg}",
            self.row_idx, self.indexer_cols_sep
        );

        let cols_len = self.parser_info.columns.len() + 1;

        let msg_cols = self
            .msg_buffer
            .split(self.indexer_cols_sep)
            .chain(std::iter::repeat(""))
            .take(cols_len);

        self.app.append_row(appender_params_from_iter(msg_cols))?;

        self.row_idx += 1;

        Ok(())
    }

    async fn flush(&mut self) -> anyhow::Result<()> {
        self.write_to_db()
    }
}

impl Drop for MsgDuckDbWriter {
    fn drop(&mut self) {
        if let Err(err) = self.write_to_db() {
            //TODO: Error should be logged and not printed to stderr.
            eprintln!("Error while writing messages to database. {err}");
        }
    }
}
