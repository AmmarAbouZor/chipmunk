mod formattable_msg;
mod ft_message_parser;
mod ft_scanner;

use std::{ops::DerefMut, sync::Mutex};

use crate::exports::host::indexer::parse_client_inter::{Error, ParseReturn};
#[allow(unused)]
use crate::host::indexer::parsing::{add, add_range, ParseYield};
use dlt_core::{dlt, parse::dlt_message};

use self::{formattable_msg::FormattableMessage, ft_scanner::FtScanner};

pub struct DltParser {
    pub with_storage_header: bool,
    ft_scanner: Mutex<FtScanner>,
}

impl DltParser {
    pub fn new() -> Self {
        Self {
            with_storage_header: true,
            ft_scanner: Mutex::new(FtScanner::new()),
        }
    }

    pub fn init(&self, _config: String) {}

    fn parse_intern(
        with_storage_header: bool,
        ft_scanner: &mut FtScanner,
        data: &[u8],
        timestamp: Option<u64>,
    ) -> Result<ParseReturn, Error> {
        match dlt_message(&data, None, with_storage_header)
            .map_err(|e| Error::Parse(format!("{e}")))?
        {
            (rest, dlt_core::parse::ParsedMessage::FilteredOut(_n)) => {
                let offset = (data.len() - rest.len()) as u64;
                // TODO AAZ: This retured rest on the original version.
                Ok(ParseReturn {
                    cursor: offset,
                    value: None,
                })
            }
            (_, dlt_core::parse::ParsedMessage::Invalid) => {
                Err(Error::Parse("Invalid parse".to_owned()))
            }
            (rest, dlt_core::parse::ParsedMessage::Item(i)) => {
                let attachment = ft_scanner.process(&i);
                let msg_with_storage_header = if i.storage_header.is_some() {
                    i
                } else {
                    i.add_storage_header(timestamp.map(dlt::DltTimeStamp::from_ms))
                };

                let msg = FormattableMessage {
                    message: msg_with_storage_header,
                }
                .to_string();

                let offset = (data.len() - rest.len()) as u64;

                let value = if let Some(attachment) = attachment {
                    Some(ParseYield::MessageAndAttachment((msg, attachment)))
                } else {
                    Some(ParseYield::Message(msg))
                };

                Ok(ParseReturn {
                    cursor: offset,
                    value,
                })
            }
        }
    }

    pub fn parse_res(&self, data: Vec<u8>, timestamp: Option<u64>) {
        let mut slice = &data[0..];
        let mut ft_scanner = self.ft_scanner.lock().unwrap();
        loop {
            match Self::parse_intern(
                self.with_storage_header,
                ft_scanner.deref_mut(),
                slice,
                timestamp,
            ) {
                Ok(res) => {
                    slice = &slice[res.cursor as usize..];

                    add(Ok(&res));
                }
                Err(err) => {
                    add(Err(&err));
                    return;
                }
            }
        }
    }

    pub fn parse(&self, data: Vec<u8>, timestamp: Option<u64>) -> Vec<Result<ParseReturn, Error>> {
        let mut results = Vec::new();
        let mut slice = &data[0..];
        let mut ft_scanner = self.ft_scanner.lock().unwrap();
        loop {
            match Self::parse_intern(
                self.with_storage_header,
                ft_scanner.deref_mut(),
                slice,
                timestamp,
            ) {
                Ok(res) => {
                    slice = &slice[res.cursor as usize..];
                    results.push(Ok(res));
                }
                Err(err) => {
                    results.push(Err(err));
                    return results;
                }
            }
        }
    }
}
