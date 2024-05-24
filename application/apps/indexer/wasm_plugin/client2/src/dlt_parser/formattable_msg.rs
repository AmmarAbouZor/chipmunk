use std::fmt::{self, Formatter};

use dlt_core::{
    dlt::{
        Argument, ControlType, DltTimeStamp, ExtendedHeader, Message, MessageType,
        NetworkTraceType, PayloadContent, StandardHeader, StorageHeader, Value,
    },
    service_id::service_id_lookup,
};

const DLT_COLUMN_SENTINAL: char = '\u{0004}';
const DLT_ARGUMENT_SENTINAL: char = '\u{0005}';

/// A dlt message that can be formatted with optional FIBEX data support
pub struct FormattableMessage {
    pub message: Message,
}

impl FormattableMessage {
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
        self.write_app_id_context_id_and_message_type(f)?;
        let _ = match get_message_type_string(&self.message.extended_header) {
            Some(v) => f.write_str(
                &format!("{DLT_ARGUMENT_SENTINAL}[{id}]{DLT_ARGUMENT_SENTINAL} {v}")[..],
            ),
            None => f.write_str(
                &format!("{DLT_ARGUMENT_SENTINAL}[{id}]{DLT_ARGUMENT_SENTINAL} {data:02X?}")[..],
            ),
        };
        Ok(())
    }
}

impl fmt::Display for FormattableMessage {
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
            write!(f, "{}", DltStorageHeader(h))?;
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
        }
    }
}

//   EColumn.DATETIME,
//   EColumn.ECUID,
struct DltStorageHeader<'a>(&'a StorageHeader);
impl<'a> fmt::Display for DltStorageHeader<'a> {
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

struct DltDltTimeStamp<'a>(&'a DltTimeStamp);

impl<'a> fmt::Display for DltDltTimeStamp<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        use chrono::{DateTime, Utc};
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

impl<'a> fmt::Display for DltStandardHeader<'a> {
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

struct DltMessageType<'a>(&'a MessageType);

impl<'a> fmt::Display for DltMessageType<'a> {
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

struct DltArgument<'a>(&'a Argument);

impl<'a> fmt::Display for DltArgument<'a> {
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

struct DltValue<'a>(&'a Value);

impl<'a> fmt::Display for DltValue<'a> {
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
