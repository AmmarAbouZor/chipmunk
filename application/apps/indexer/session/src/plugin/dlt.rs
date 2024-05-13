use parsers::{Attachment, Error, ParseYield, Parser};

use super::{PluginParseMessage, PluginParser};

use plugin_host::dlt_parser::{
    Attachment as PluginAttachment, Error as PluginErr, ParseYield as PluginYield,
};

pub struct DltWasmParser {
    parser: plugin_host::DltParser,
}

//TODO AAZ: Make sure we release the resource in WASM when the parser is dropped

impl PluginParser for DltWasmParser {
    fn create(config_path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let parser = plugin_host::DltParser::create(config_path).map_err(|err| err.to_string())?;

        Ok(Self { parser })
    }
}

impl Parser<PluginParseMessage> for DltWasmParser {
    fn parse<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<ParseYield<PluginParseMessage>>), Error> {
        let res = self
            .parser
            .parse(input, timestamp)
            .map_err(|err| parsers::Error::Parse(err.to_string()))?;

        match res {
            Ok(val) => {
                let remain = &input[val.cursor as usize..];
                let yld = match val.value {
                    Some(PluginYield::Message(msg)) => Some(ParseYield::Message(msg.into())),
                    Some(PluginYield::Attachment(att)) => {
                        Some(ParseYield::Attachment(plugin_to_host_attachment(att)))
                    }
                    Some(PluginYield::MessageAndAttachment((msg, att))) => {
                        Some(ParseYield::MessageAndAttachment((
                            msg.into(),
                            plugin_to_host_attachment(att),
                        )))
                    }
                    None => None,
                };
                Ok((remain, yld))
            }
            Err(e) => match e {
                PluginErr::Parse(msg) => Err(Error::Parse(msg)),
                PluginErr::Incomplete => Err(Error::Incomplete),
                PluginErr::Eof => Err(Error::Eof),
            },
        }
    }
}

fn plugin_to_host_attachment(plug_att: PluginAttachment) -> Attachment {
    Attachment {
        data: plug_att.data,
        name: plug_att.name,
        size: plug_att.size as usize,
        messages: plug_att.messages.into_iter().map(|n| n as usize).collect(),
        created_date: plug_att.created_date,
        modified_date: plug_att.modified_date,
    }
}
