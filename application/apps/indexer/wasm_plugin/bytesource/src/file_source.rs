use std::{
    cell::RefCell,
    fs::File,
    io::{Read, Seek},
    path::PathBuf,
};

use crate::{
    exports::host::indexer::source_client::GuestByteSource, host::indexer::sourcing::SourceError,
};

//TODO AAZ: Taken from native. Make sure we need it here.
// pub(crate) const DEFAULT_READER_CAPACITY: usize = 10 * 1024 * 1024;

// The structure must be the same between the host and the guest
const WASM_FILES_DIR: &str = "./files";

pub struct FileSource {
    file: RefCell<Option<File>>,
}

impl GuestByteSource for FileSource {
    fn new(_config_path: String) -> Self {
        Self {
            file: RefCell::new(None),
        }
    }

    fn init(&self, file_name: String) -> Result<(), SourceError> {
        let file_path = PathBuf::from(WASM_FILES_DIR).join(file_name);
        let file = File::open(file_path).map_err(|err| SourceError::Io(err.to_string()))?;
        *self.file.borrow_mut() = Some(file);
        Ok(())
    }

    fn read(&self, len: u64) -> Result<Vec<u8>, SourceError> {
        let mut buf = Vec::with_capacity(len as usize);
        let mut file_borrow = self.file.borrow_mut();
        let file = file_borrow
            .as_mut()
            .ok_or_else(|| SourceError::Other("Source is not initialized".into()))?;

        file.read_exact(&mut buf)
            .map_err(|err| SourceError::Io(format!("Error while reading from file: {}", err)))?;

        Ok(buf)
    }

    fn reload(&self) -> Result<(), SourceError> {
        let mut file_borrow = self.file.borrow_mut();
        let file = file_borrow
            .as_mut()
            .ok_or_else(|| SourceError::Other("Source is not initialized".into()))?;

        file.rewind()
            .map_err(|err| SourceError::Io(err.to_string()))
    }
}
