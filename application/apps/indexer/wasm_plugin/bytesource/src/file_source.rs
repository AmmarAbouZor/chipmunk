use crate::{
    exports::host::indexer::source_client::GuestByteSource, host::indexer::sourcing::SourceError,
};

pub struct FileSource;

impl GuestByteSource for FileSource {
    fn new(config_path: String) -> Self {
        todo!()
    }

    fn read(&self, len: u64) -> Result<Vec<u8>, SourceError> {
        todo!()
    }

    fn reload(&self) -> Result<(), SourceError> {
        todo!()
    }
}
