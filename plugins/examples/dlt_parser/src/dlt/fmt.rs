//! # Formatting dlt messages as text
use chrono::prelude::{DateTime, Utc};
use chrono_tz::Tz;
use dlt_core::{
    dlt::{
        Argument, ControlType, DltTimeStamp, ExtendedHeader, LogLevel, Message, MessageType,
        NetworkTraceType, PayloadContent, StandardHeader, StorageHeader, StringCoding, TypeInfo,
        TypeInfoKind, Value,
    },
    fibex::{extract_metadata, FibexMetadata as FibexDltMetadata},
    parse::construct_arguments,
    service_id::service_id_lookup,
};
use plugins_api::log::trace;

use std::fmt::{self, Formatter};

//TODO AAZ: Remove these and use delimiters from configs.

/// Separator to used between the columns in DLT [`FormattableMessage`].
pub const DLT_COLUMN_SENTINAL: char = '\u{0004}';
/// Separator to used between the arguments in the payload of DLT [`FormattableMessage`].
pub const DLT_ARGUMENT_SENTINAL: char = '\u{0005}';

fn try_new_from_fibex_message_info(message_info: &str) -> Option<MessageType> {
    Some(MessageType::Log(match message_info {
        "DLT_LOG_FATAL" => LogLevel::Fatal,
        "DLT_LOG_ERROR" => LogLevel::Error,
        "DLT_LOG_WARN" => LogLevel::Warn,
        "DLT_LOG_INFO" => LogLevel::Info,
        "DLT_LOG_DEBUG" => LogLevel::Debug,
        "DLT_LOG_VERBOSE" => LogLevel::Verbose,
        _ => return None,
    }))
}

struct DltMessageType<'a>(&'a MessageType);

impl fmt::Display for DltMessageType<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match &self.0 {
            MessageType::ApplicationTrace(app_type) => write!(f, "{} ", app_type.as_ref()),
            MessageType::Control(c) => write!(f, "{}", c.as_ref()),
            MessageType::Log(log_level) => write!(f, "{}", log_level.as_ref()),
            MessageType::NetworkTrace(trace_type) => write!(f, "{}", trace_type.as_ref()),
            MessageType::Unknown(v) => write!(f, "Unknown message type ({},{})", v.0, v.1),
        }
    }
}
//   EColumn.DATETIME,
//   EColumn.ECUID,
struct DltStorageHeader<'a>(&'a StorageHeader);
impl fmt::Display for DltStorageHeader<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}{}{}",
            DltDltTimeStamp(&self.0.timestamp),
            DLT_COLUMN_SENTINAL,
            self.0.ecu_id
        )
    }
}

struct DltValue<'a>(&'a Value);

impl fmt::Display for DltValue<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match &self.0 {
            Value::Bool(value) => value.fmt(f),
            Value::U8(value) => value.fmt(f),
            Value::U16(value) => value.fmt(f),
            Value::U32(value) => value.fmt(f),
            Value::U64(value) => value.fmt(f),
            Value::U128(value) => value.fmt(f),
            Value::I8(value) => value.fmt(f),
            Value::I16(value) => value.fmt(f),
            Value::I32(value) => value.fmt(f),
            Value::I64(value) => value.fmt(f),
            Value::I128(value) => value.fmt(f),
            Value::F32(value) => value.fmt(f),
            Value::F64(value) => value.fmt(f),
            Value::StringVal(s) => {
                for line in s.lines() {
                    write!(f, "{line}")?;
                }
                Ok(())
            }
            Value::Raw(value) => write!(f, "{value:02X?}"),
        }
    }
}

struct DltArgument<'a>(&'a Argument);

impl fmt::Display for DltArgument<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        if let Some(n) = &self.0.name {
            write!(f, "{n}")?;
        }
        if let Some(u) = &self.0.unit {
            write!(f, "{u}")?;
        }
        if let Some(v) = self.0.to_real_value() {
            write!(f, "{v}")?;
        } else {
            write!(f, "{}", DltValue(&self.0.value))?;
        }

        Ok(())
    }
}

struct DltDltTimeStamp<'a>(&'a DltTimeStamp);

impl fmt::Display for DltDltTimeStamp<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let dt: Option<DateTime<Utc>> =
            DateTime::from_timestamp(i64::from(self.0.seconds), self.0.microseconds * 1000);
        match dt {
            Some(dt) => {
                let system_time: std::time::SystemTime = std::time::SystemTime::from(dt);
                write!(f, "{}", humantime::format_rfc3339(system_time))
            }
            None => write!(
                f,
                "no valid timestamp for {}s/{}us",
                self.0.seconds, self.0.microseconds
            ),
        }
    }
}

//   EColumn.DATETIME,
//   EColumn.ECUID,
struct DltStandardHeader<'a>(&'a StandardHeader);

impl fmt::Display for DltStandardHeader<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}{}", self.0.version, DLT_COLUMN_SENTINAL)?;
        if let Some(id) = &self.0.session_id {
            write!(f, "{id}")?;
        }
        write!(
            f,
            "{}{}{}",
            DLT_COLUMN_SENTINAL, self.0.message_counter, DLT_COLUMN_SENTINAL,
        )?;
        if let Some(t) = &self.0.timestamp {
            write!(f, "{t}")?;
        }
        write!(f, "{DLT_COLUMN_SENTINAL}",)?;
        if let Some(id) = &self.0.ecu_id {
            write!(f, "{id}")?;
        }
        Ok(())
    }
}

#[derive(Default, Debug, Clone)]
pub struct FormatOptions {
    pub tz: Option<Tz>,
}

impl From<Option<&String>> for FormatOptions {
    fn from(value: Option<&String>) -> Self {
        FormatOptions {
            tz: if let Some(tz) = value {
                tz.parse::<Tz>().map_or(None, Option::from)
            } else {
                None
            },
        }
    }
}

/// A dlt message that can be formatted with optional FIBEX data support
pub struct FormattableMessage<'a> {
    pub message: Message,
    pub fibex_dlt_metadata: Option<&'a FibexDltMetadata>,
    pub options: Option<&'a FormatOptions>,
}

impl From<Message> for FormattableMessage<'_> {
    fn from(message: Message) -> Self {
        FormattableMessage {
            message,
            fibex_dlt_metadata: None,
            options: None,
        }
    }
}

impl FormattableMessage<'_> {
    fn write_app_id_context_id_and_message_type(
        &self,
        f: &mut fmt::Formatter,
    ) -> Result<(), fmt::Error> {
        match self.message.extended_header.as_ref() {
            Some(ext) => {
                write!(
                    f,
                    "{}{}{}{}{}{}",
                    ext.application_id,
                    DLT_COLUMN_SENTINAL,
                    ext.context_id,
                    DLT_COLUMN_SENTINAL,
                    DltMessageType(&ext.message_type),
                    DLT_COLUMN_SENTINAL,
                )?;
            }
            None => {
                write!(
                    f,
                    "-{DLT_COLUMN_SENTINAL}-{DLT_COLUMN_SENTINAL}-{DLT_COLUMN_SENTINAL}",
                )?;
            }
        };
        Ok(())
    }
    pub(crate) fn format_nonverbose_data(
        &self,
        id: u32,
        data: &[u8],
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        trace!("format_nonverbose_data");
        let mut fibex_info_added = false;
        if let Some(non_verbose_info) = self.info_from_metadata(id, data) {
            write!(
                f,
                "{}{}{}{}",
                non_verbose_info.app_id.unwrap_or("-"),
                DLT_COLUMN_SENTINAL,
                non_verbose_info.context_id.unwrap_or("-"),
                DLT_COLUMN_SENTINAL,
            )?;
            if let Some(v) = non_verbose_info.msg_type {
                write!(f, "{}", DltMessageType(&v))?;
            } else {
                write!(f, "-")?;
            }
            write!(f, "{DLT_COLUMN_SENTINAL}")?;
            fibex_info_added = !non_verbose_info.arguments.is_empty();
            for arg in non_verbose_info.arguments {
                write!(f, "{}{} ", DLT_ARGUMENT_SENTINAL, DltArgument(&arg))?;
            }
        } else {
            self.write_app_id_context_id_and_message_type(f)?;
        }
        if !fibex_info_added {
            let _ =
                match get_message_type_string(&self.message.extended_header) {
                    Some(v) => f.write_str(
                        &format!("{DLT_ARGUMENT_SENTINAL}[{id}]{DLT_ARGUMENT_SENTINAL} {v}")[..],
                    ),
                    None => f.write_str(
                        &format!(
                            "{DLT_ARGUMENT_SENTINAL}[{id}]{DLT_ARGUMENT_SENTINAL} {data:02X?}"
                        )[..],
                    ),
                };
        }
        Ok(())
    }

    fn info_from_metadata<'b>(&'b self, id: u32, data: &[u8]) -> Option<NonVerboseInfo<'b>> {
        let fibex = self.fibex_dlt_metadata?;
        let md = extract_metadata(fibex, id, self.message.extended_header.as_ref())?;
        let msg_type: Option<MessageType> = message_type(&self.message, md.message_info.as_deref());
        let app_id = md.application_id.as_deref().or_else(|| {
            self.message
                .extended_header
                .as_ref()
                .map(|h| h.application_id.as_str())
        });
        let context_id = md.context_id.as_deref().or_else(|| {
            self.message
                .extended_header
                .as_ref()
                .map(|h| h.context_id.as_ref())
        });
        let mut arguments = vec![];
        for pdu in &md.pdus {
            if let Some(description) = &pdu.description {
                let arg = Argument {
                    type_info: TypeInfo {
                        kind: TypeInfoKind::StringType,
                        coding: StringCoding::UTF8,
                        has_trace_info: false,
                        has_variable_info: false,
                    },
                    name: None,
                    unit: None,
                    fixed_point: None,
                    value: Value::StringVal(description.to_string()),
                };
                arguments.push(arg);
            } else {
                if let Ok(mut new_args) =
                    construct_arguments(self.message.header.endianness, &pdu.signal_types, data)
                {
                    arguments.append(&mut new_args);
                }
                trace!("Constructed {} arguments", arguments.len());
            };
        }
        Some(NonVerboseInfo {
            app_id,
            context_id,
            msg_type,
            arguments,
        })
    }
}

impl fmt::Display for FormattableMessage<'_> {
    /// will format dlt Message with those fields:
    /// ********* storage-header ********
    /// date-time
    /// ecu-id (skip...contained in header section)
    /// ********* header ********
    /// Version
    /// message-counter
    /// timestamp
    /// ecu id
    /// session-id
    /// ********* ext-header ********
    /// message-type
    /// app-id
    /// context-id
    ///
    /// payload
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        if let Some(h) = &self.message.storage_header {
            let tz = self.options.map(|o| o.tz);
            match tz {
                Some(Some(tz)) => {
                    write_tz_string(f, &h.timestamp, &tz)?;
                    write!(f, "{DLT_COLUMN_SENTINAL}{}", h.ecu_id)?;
                }
                _ => write!(f, "{}", DltStorageHeader(h))?,
            };
        }
        let header = DltStandardHeader(&self.message.header);
        write!(f, "{DLT_COLUMN_SENTINAL}",)?;
        write!(f, "{header}")?;
        write!(f, "{DLT_COLUMN_SENTINAL}",)?;

        match &self.message.payload {
            PayloadContent::Verbose(arguments) => {
                self.write_app_id_context_id_and_message_type(f)?;
                arguments
                    .iter()
                    .try_for_each(|arg| write!(f, "{}{}", DLT_ARGUMENT_SENTINAL, DltArgument(arg)))
            }
            PayloadContent::NonVerbose(id, data) => self.format_nonverbose_data(*id, data, f),
            PayloadContent::ControlMsg(ctrl_id, _data) => {
                self.write_app_id_context_id_and_message_type(f)?;
                match service_id_lookup(ctrl_id.value()) {
                    Some((name, _desc)) => write!(f, "[{name}]"),
                    None => write!(f, "[Unknown CtrlCommand]"),
                }
            }
            PayloadContent::NetworkTrace(slices) => {
                self.write_app_id_context_id_and_message_type(f)?;

                slices
                    .iter()
                    .try_for_each(|slice| write!(f, "{}{:02X?}", DLT_ARGUMENT_SENTINAL, slice))
            }
        }
    }
}

fn write_tz_string(
    f: &mut Formatter,
    time_stamp: &DltTimeStamp,
    tz: &Tz,
) -> Result<(), fmt::Error> {
    let dt: Option<DateTime<Utc>> = DateTime::from_timestamp(
        i64::from(time_stamp.seconds),
        time_stamp.microseconds * 1000,
    );
    match dt {
        Some(dt) => write!(f, "{}", dt.with_timezone(tz)),
        None => write!(
            f,
            "no valid timestamp for {}s/{}us",
            time_stamp.seconds, time_stamp.microseconds,
        ),
    }
}

fn message_type(msg: &Message, message_info: Option<&str>) -> Option<MessageType> {
    if let Some(v) = message_info
        .as_ref()
        .and_then(|mi| try_new_from_fibex_message_info(mi))
    {
        Some(v)
    } else {
        msg.extended_header.as_ref().map(|h| h.message_type.clone())
    }
}

fn get_message_type_string(extended_header: &Option<ExtendedHeader>) -> Option<&str> {
    if let Some(ext) = extended_header {
        match &ext.message_type {
            MessageType::Control(ct) => match ct {
                ControlType::Request => Some("control request"),
                ControlType::Response => Some("control response"),
                ControlType::Unknown(_) => Some("unknown control"),
            },
            MessageType::NetworkTrace(ntt) => match ntt {
                NetworkTraceType::Ipc => Some("Ipc"),
                NetworkTraceType::Can => Some("Can"),
                NetworkTraceType::Flexray => Some("Flexray"),
                NetworkTraceType::Most => Some("Most"),
                NetworkTraceType::Ethernet => Some("Ethernet"),
                NetworkTraceType::Someip => Some("Someip"),
                NetworkTraceType::Invalid => Some("Invalid"),
                _ => Some("unknown network trace"),
            },
            _ => None,
        }
    } else {
        None
    }
}

struct NonVerboseInfo<'a> {
    app_id: Option<&'a str>,
    context_id: Option<&'a str>,
    msg_type: Option<MessageType>,
    arguments: Vec<Argument>,
}
