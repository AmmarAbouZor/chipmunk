use std::collections::HashMap;

use dlt_core::dlt::Message;

use crate::host::indexer::parsing::Attachment;

use super::ft_message_parser::{FtMessage, FtMessageParser};

/// An scanner for DLT-FT files contained in a DLT trace.
#[derive(Debug)]
pub struct FtScanner {
    files: HashMap<u32, Attachment>,
    index: usize,
}

impl FtScanner {
    /// Creates a new scanner.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            index: 0,
        }
    }

    /// Processes the next DLT message of the trace.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to be processed.
    pub fn process(&mut self, message: &Message) -> Option<Attachment> {
        let result = if let Some(ft_message) = FtMessageParser::parse(message) {
            match ft_message {
                FtMessage::Start(ft_start) => {
                    self.files.insert(
                        ft_start.id,
                        Attachment {
                            name: ft_start.name.clone(),
                            size: ft_start.size as u64,
                            created_date: Some(ft_start.created),
                            modified_date: None,
                            messages: vec![self.index as u64],
                            data: Vec::new(),
                        },
                    );
                    None
                }
                FtMessage::Data(ft_data) => {
                    if let Some(mut ft_file) = self.files.remove(&ft_data.id) {
                        ft_file.messages.push(self.index as u64);
                        ft_file.data.extend_from_slice(ft_data.bytes);
                        self.files.insert(ft_data.id, ft_file);
                    }
                    None
                }
                FtMessage::End(ft_end) => {
                    if let Some(mut ft_file) = self.files.remove(&ft_end.id) {
                        ft_file.messages.push(self.index as u64);
                        Some(ft_file)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        };
        self.index += 1;
        result
    }
}
