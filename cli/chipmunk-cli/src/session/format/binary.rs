//! Structures and methods to write parsed message in binary format.

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::Context;
use parsers::LogMessage;

use crate::session::create_append_file_writer;

use super::MessageWriter;

/// Structure to write parsed message in binary format.
#[derive(Debug)]
pub struct MsgBinaryWriter {
    /// Buffer writer to the output file.
    output_file: BufWriter<File>,
}

impl MsgBinaryWriter {
    /// Creates a new instance with the given arguments.
    ///
    /// * `output_file`: The path for the output file to write the message to.
    pub fn new(output_file: &Path) -> anyhow::Result<Self> {
        let output_file = create_append_file_writer(output_file)?;
        let writer = Self { output_file };

        Ok(writer)
    }
}

impl MessageWriter for MsgBinaryWriter {
    fn write_msg<M>(&mut self, msg: &M) -> anyhow::Result<()>
    where
        M: LogMessage,
    {
        msg.to_writer(&mut self.output_file)
            .context("Error while writing binary message")?;

        Ok(())
    }

    async fn flush(&mut self) -> anyhow::Result<()> {
        self.output_file
            .flush()
            .context("Error while writing to output file")?;

        Ok(())
    }
}
