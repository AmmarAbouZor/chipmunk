use std::{
    cell::RefCell,
    fs::File,
    io::{BufReader, Read},
};

use crate::{
    exports::host::indexer::source_client::GuestByteSource, host::indexer::sourcing::SourceError,
};

#[derive(Default)]
pub struct FileSource {
    reader: RefCell<Option<BufReader<File>>>,
}

impl GuestByteSource for FileSource {
    fn new() -> Self {
        Self::default()
    }

    fn init(&self, _config_path: String, file_path: String) -> Result<(), SourceError> {
        let file = File::open(file_path).map_err(|err| SourceError::Io(err.to_string()))?;
        *self.reader.borrow_mut() = Some(BufReader::new(file));
        Ok(())
    }

    fn read(&self, len: u64) -> Result<Vec<u8>, SourceError> {
        let len = len as usize;
        let mut buf = Vec::with_capacity(len);
        //TODO AAZ: Test the difference for this unsafe code call.
        // SAFETY: truncate is called on the buffer after read call with the read amount of bytes.
        unsafe {
            buf.set_len(len);
        }

        let mut reader_borrow = self.reader.borrow_mut();
        let reader = reader_borrow
            .as_mut()
            .ok_or_else(|| SourceError::Other("Source is not initialized".into()))?;

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

        Ok(buf)
    }
}
