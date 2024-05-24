mod formattable_msg;
mod ft_message_parser;
mod ft_scanner;

use std::cell::Cell;
use std::{cell::RefCell, ops::DerefMut};

use crate::exports::host::indexer::parse2_client::{Error, GuestParser, ParseReturn};
use crate::host::indexer::parsing::ParseYield;
use crate::host::indexer::source_general::ByteSource;
use dlt_core::{dlt, parse::dlt_message};

use self::{formattable_msg::FormattableMessage, ft_scanner::FtScanner};

pub struct DltParser {
    pub with_storage_header: bool,
    ft_scanner: RefCell<FtScanner>,
    data: RefCell<Option<Vec<u8>>>,
    cursor: Cell<usize>,
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
                // TODO AAZ: This return rest on the original version.
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

impl DltParser {
    #[inline]
    fn parse_next_intern(
        &self,
        data_opt: &mut Option<Vec<u8>>,
        ft_scanner: &mut FtScanner,
        source: &ByteSource,
        timestamp: Option<u64>,
    ) -> Result<ParseReturn, Error> {
        let data = match data_opt.as_ref() {
            Some(data) => data,
            None => {
                self.cursor.set(0);
                let bytes = source.read_next().map_err(|err| {
                    Error::Parse(format!("Error while loading data. TODO TEMP. Error: {err}"))
                })?;
                *data_opt = Some(bytes);
                data_opt.as_ref().unwrap()
            }
        };

        let slice = &data[self.cursor.get()..];

        //TODO AAZ: Temp solution because it's possible that the current chunk ends with a whole
        //item by coincidence even the source isn't complete yet.
        if slice.len() == 0 {
            return Err(Error::Eof);
        }

        match Self::parse_intern(self.with_storage_header, ft_scanner, slice, timestamp) {
            Ok(res) => {
                let mut cursor = self.cursor.get();
                cursor += res.cursor as usize;
                self.cursor.set(cursor);
                Ok(res)
            }
            Err(_) => {
                // Load more data from source.
                *data_opt = None;
                self.parse_next_intern(data_opt, ft_scanner, source, timestamp)
            }
        }
    }
}

impl GuestParser for DltParser {
    fn new() -> Self {
        Self {
            with_storage_header: true,
            ft_scanner: RefCell::new(FtScanner::new()),
            data: RefCell::new(None),
            cursor: Cell::new(0),
        }
    }

    fn parse_next(
        &self,
        source: &ByteSource,
        timestamp: Option<u64>,
    ) -> Result<ParseReturn, Error> {
        let mut data_opt = self.data.borrow_mut();
        let data_opt = data_opt.deref_mut();
        let mut ft_scanner = self.ft_scanner.borrow_mut();

        self.parse_next_intern(data_opt, ft_scanner.deref_mut(), source, timestamp)
    }
}
