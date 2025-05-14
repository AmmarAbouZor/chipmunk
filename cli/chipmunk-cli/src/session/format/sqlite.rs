use std::path::Path;

use anyhow::Context;

use crate::session::parser::ParserInfo;
use super::MessageWriter;

/// Structure to write parsed message in into SQLite database file.
#[derive(Debug)]
pub struct MsgSqliteWriter {}

impl MsgSqliteWriter {
    /// Creates a new instance with the given arguments.
    ///
    /// * `output_file`: The path for the output database file to write the message to.
    pub async fn new(_output_file: &Path, _parser_info: ParserInfo) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

impl MessageWriter for MsgSqliteWriter {
    fn write_msg<M>(&mut self, _msg: &M) -> anyhow::Result<()>
    where
        M: parsers::LogMessage,
    {
        todo!()
    }

    async fn flush(&mut self) -> anyhow::Result<()> {
        todo!()
    }
}
