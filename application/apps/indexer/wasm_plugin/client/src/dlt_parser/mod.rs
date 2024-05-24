mod formattable_msg;
mod ft_message_parser;
mod ft_scanner;

use std::{cell::RefCell, ops::DerefMut};

use crate::exports::host::indexer::parse_client::{Error, GuestParser, ParseReturn, Results};
use crate::host::indexer::parsing::ParseYield;
use dlt_core::{dlt, parse::dlt_message};

use self::{formattable_msg::FormattableMessage, ft_scanner::FtScanner};

pub struct DltParser {
    pub with_storage_header: bool,
    ft_scanner: RefCell<FtScanner>,
}

impl DltParser {
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
}

impl GuestParser for DltParser {
    fn new() -> Self {
        Self {
            with_storage_header: true,
            ft_scanner: RefCell::new(FtScanner::new()),
        }
    }

    fn parse_res(&self, data: Vec<u8>, timestamp: Option<u64>, results: &Results) {
        let mut slice = &data[0..];
        let mut ft_scanner = self.ft_scanner.borrow_mut();
        loop {
            match Self::parse_intern(
                self.with_storage_header,
                ft_scanner.deref_mut(),
                slice,
                timestamp,
            ) {
                Ok(res) => {
                    slice = &slice[res.cursor as usize..];
                    results.add(Ok(&res));
                }
                Err(err) => {
                    results.add(Err(&err));
                    return;
                }
            }
        }
    }

    #[allow(unused)]
    fn parse_res_rng(&self, data: Vec<u8>, timestamp: Option<u64>, results: &Results) {
        let mut items = Vec::new();
        let mut slice = &data[0..];
        let mut ft_scanner = self.ft_scanner.borrow_mut();
        loop {
            match Self::parse_intern(
                self.with_storage_header,
                ft_scanner.deref_mut(),
                slice,
                timestamp,
            ) {
                Ok(res) => {
                    slice = &slice[res.cursor as usize..];
                    items.push(Ok(res));
                }
                Err(err) => {
                    items.push(Err(err));
                    break;
                }
            }
        }
        unreachable!("res range is activated in favor of res single");

        // results.add_range(&items);
    }

    fn parse(&self, data: Vec<u8>, timestamp: Option<u64>) -> Vec<Result<ParseReturn, Error>> {
        let mut results = Vec::new();
        let mut slice = &data[0..];
        let mut ft_scanner = self.ft_scanner.borrow_mut();
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
