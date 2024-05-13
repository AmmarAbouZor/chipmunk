mod formattable_msg;
mod ft_message_parser;
mod ft_scanner;

use std::cell::RefCell;

use dlt_core::{dlt, parse::dlt_message};

use crate::exports::host::parse::parsing::{self, Error, GuestParser, ParseReturn, ParseYield};

use self::{formattable_msg::FormattableMessage, ft_scanner::FtScanner};

pub struct DltParser {
    pub with_storage_header: bool,
    ft_scanner: RefCell<FtScanner>,
}

impl GuestParser for DltParser {
    fn new() -> Self {
        Self {
            with_storage_header: false,
            ft_scanner: RefCell::new(FtScanner::new()),
        }
    }

    fn parse(&self, data: Vec<u8>, timestamp: Option<u64>) -> Result<ParseReturn, parsing::Error> {
        match dlt_message(&data, None, self.with_storage_header)
            .map_err(|e| Error::Parse(format!("{e}")))?
        {
            (rest, dlt_core::parse::ParsedMessage::FilteredOut(_n)) => {
                let offset = (data.len() - rest.len()) as u64;
                // TODO AAZ: This retured rest on the original version.
                // Ok((rest, None))
                Ok(ParseReturn {
                    cursor: offset,
                    value: None,
                })
            }
            (_, dlt_core::parse::ParsedMessage::Invalid) => {
                Err(Error::Parse("Invalid parse".to_owned()))
            }
            (rest, dlt_core::parse::ParsedMessage::Item(i)) => {
                let attachment = self.ft_scanner.borrow_mut().process(&i);
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
