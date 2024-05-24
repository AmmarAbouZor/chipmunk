use std::{
    cell::{Cell, RefCell},
    fs::File,
    io::{BufReader, Read},
    usize,
};

use crate::{
    exports::host::indexer::source_prod_client::GuestSourceProd,
    host::indexer::{parse_client::Parser, parsing::Results, sourcing::SourceError},
};

pub struct FileSourceProd {
    reader: RefCell<Option<BufReader<File>>>,
    parser: Parser,
    last_read_len: Cell<u64>,
}

impl GuestSourceProd for FileSourceProd {
    fn new() -> Self {
        let parser = Parser::new();
        Self {
            reader: Default::default(),
            parser,
            last_read_len: Cell::new(0),
        }
    }

    fn init(&self, _config_path: String, file_path: String) -> Result<(), SourceError> {
        let file = File::open(file_path).map_err(|err| SourceError::Io(err.to_string()))?;
        *self.reader.borrow_mut() = Some(BufReader::new(file));
        Ok(())
    }

    fn read_then_parse(
        &self,
        len: u64,
        read_len: u64,
        timestamp: Option<u64>,
        results: &Results,
    ) -> Result<(), SourceError> {
        let mut reader_borrow = self.reader.borrow_mut();
        let reader = reader_borrow
            .as_mut()
            .ok_or_else(|| SourceError::Other("Source is not initialized".into()))?;

        let last_read = self.last_read_len.get();
        // TODO AAZ: This should be an Option if we stayed with this approach
        if last_read > 0 && read_len > 0 {
            let remain = last_read.checked_sub(read_len).unwrap() as i64;

            reader.seek_relative(-remain).map_err(|err| {
                SourceError::Io(format!("Error while seeking in file buffer: {}", err))
            })?;
        }
        let len = len as usize;

        let mut buf = Vec::with_capacity(len);
        //TODO AAZ: Test the difference for this unsafe code call.
        // SAFETY: truncate is called on the buffer after read call with the read amount of bytes.
        unsafe {
            buf.set_len(len);
        }

        let bytes_read = reader
            .read(&mut buf)
            .map_err(|err| SourceError::Io(format!("Error while reading from file: {}", err)))?;

        // TODO AAZ: Measure this
        // This check is useful in our use case since truncate implementation checks for greater
        // than and not greater equal, but in our case we expect the bytes_read to be equal to the
        // given len in most cases.
        if bytes_read < len {
            buf.truncate(bytes_read);
        }

        self.last_read_len.set(bytes_read as u64);

        self.parser.parse_res(&buf, timestamp, results);

        Ok(())
    }
}
