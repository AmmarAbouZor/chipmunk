use std::cell::RefCell;

use parsers::{dlt::DltParser, Parser};

use crate::exports::host::parse::parsing::{
    Attachment, Error, GuestParser, ParseReturn, ParseYield,
};

pub struct WasiDltParser {
    parser: RefCell<DltParser<'static>>,
}

impl GuestParser for WasiDltParser {
    fn new() -> Self {
        let mut parser = DltParser::default();
        parser.with_storage_header = true;

        Self {
            parser: RefCell::new(parser),
        }
    }

    fn parse(&self, data: Vec<u8>, timestamp: Option<u64>) -> Result<ParseReturn, Error> {
        match self.parser.borrow_mut().parse(&data, timestamp) {
            Ok((remain, opt)) => {
                let offset = (data.len() - remain.len()) as u64;
                let ret_val = match opt {
                    Some(yld) => match yld {
                        parsers::ParseYield::Message(msg) => {
                            Some(ParseYield::Message(msg.to_string()))
                        }
                        parsers::ParseYield::Attachment(att) => {
                            Some(ParseYield::Attachment(att.into()))
                        }
                        parsers::ParseYield::MessageAndAttachment((msg, att)) => Some(
                            ParseYield::MessageAndAttachment((msg.to_string(), att.into())),
                        ),
                    },
                    None => None,
                };

                Ok(ParseReturn {
                    cursor: offset,
                    value: ret_val,
                })
            }
            Err(err) => Err(err.into()),
        }
    }
}

impl From<parsers::Attachment> for Attachment {
    fn from(att: parsers::Attachment) -> Self {
        Self {
            data: att.data,
            name: att.name,
            size: att.size as u64,
            messages: att.messages.into_iter().map(|m| m as u64).collect(),
            created_date: att.created_date,
            modified_date: att.modified_date,
        }
    }
}

impl From<parsers::Error> for Error {
    fn from(err: parsers::Error) -> Self {
        match err {
            parsers::Error::Parse(msg) => Error::Parse(msg),
            parsers::Error::Incomplete => Error::Incomplete,
            parsers::Error::Eof => Error::Eof,
        }
    }
}
