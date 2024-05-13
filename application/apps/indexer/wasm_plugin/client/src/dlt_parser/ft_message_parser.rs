use dlt_core::dlt::Value;
use dlt_core::dlt::{Argument, LogLevel, Message, MessageType, PayloadContent};

const FT_START_TAG: &str = "FLST";
const FT_DATA_TAG: &str = "FLDA";
const FT_END_TAG: &str = "FLFI";

/// List of DLT-FT messages.
#[derive(Debug, PartialEq, Eq)]
pub enum FtMessage<'a> {
    /// Item for a DLT-FT start message.
    Start(FileStart),
    /// Item for a DLT-FT data message.
    Data(FileData<'a>),
    /// Item for a DLT-FT end message.
    End(FileEnd),
}

/// A DLT-FT start message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStart {
    /// The timestamp of the DLT message, if any.
    pub timestamp: Option<u32>,
    /// The id of the file.
    pub id: u32,
    /// The name of the file.
    pub name: String,
    /// The total size of the file.
    pub size: u32,
    /// The creation date of the file.
    pub created: String,
    /// The total number of packets.
    pub packets: u32,
}

/// A DLT-FT data message.
#[derive(Debug, PartialEq, Eq)]
pub struct FileData<'a> {
    /// The timestamp of the DLT message, if any.
    pub timestamp: Option<u32>,
    /// The id of the file.
    pub id: u32,
    /// The index of the packet (1-based).
    pub packet: u32,
    /// The payload of the packet.
    pub bytes: &'a Vec<u8>,
}

/// A DLT-FT end message.
#[derive(Debug, PartialEq, Eq)]
pub struct FileEnd {
    /// The timestamp of the DLT message, if any.
    pub timestamp: Option<u32>,
    /// The id of the file.
    pub id: u32,
}

/// A parser for DLT-FT messages.
pub struct FtMessageParser;

impl FtMessageParser {
    /// Parses a DLT-FT message from a DLT message, if any.
    pub fn parse(message: &Message) -> Option<FtMessage> {
        if let MessageType::Log(LogLevel::Info) = message.extended_header.as_ref()?.message_type {
            if let PayloadContent::Verbose(args) = &message.payload {
                if args.len() > 2 {
                    if let (Some(arg1), Some(arg2)) = (args.first(), args.last()) {
                        if Self::is_kind_of(FT_START_TAG, arg1, arg2) {
                            return Self::start_message(message.header.timestamp, args);
                        } else if Self::is_kind_of(FT_DATA_TAG, arg1, arg2) {
                            return Self::data_message(message.header.timestamp, args);
                        } else if Self::is_kind_of(FT_END_TAG, arg1, arg2) {
                            return Self::end_message(message.header.timestamp, args);
                        }
                    }
                }
            }
        }

        None
    }

    /// Returns weather both arguments contain given tag.
    fn is_kind_of(tag: &str, arg1: &Argument, arg2: &Argument) -> bool {
        if let Some(string1) = Self::get_string(arg1) {
            if let Some(string2) = Self::get_string(arg2) {
                return (string1 == tag) && (string2 == tag);
            }
        }

        false
    }

    /// Parses a DLT-FT start message from a DLT argument list, if any.
    ///
    /// # Expected arguments:
    ///
    /// * 0 DLT_STRING("FLST")
    /// * 1 DLT_UINT(file-id)
    /// * 2 DLT_STRING(file-name)
    /// * 3 DLT_UINT(file-size)
    /// * 4 DLT_STRING(date-created)
    /// * 5 DLT_UINT(packets-count)
    /// * 6 DLT_UINT(buffer-size)
    /// * 7 DLT_STRING("FLST")
    fn start_message(timestamp: Option<u32>, args: &[Argument]) -> Option<FtMessage> {
        Some(FtMessage::Start(FileStart {
            timestamp,
            id: Self::get_number(args.get(1)?)?,
            name: Self::get_string(args.get(2)?)?,
            size: Self::get_number(args.get(3)?)?,
            created: Self::get_string(args.get(4)?)?,
            packets: Self::get_number(args.get(5)?)?,
        }))
    }

    /// Returns a DLT-FT data message from a DLT argument list, if any.
    ///
    /// # Expected arguments:
    ///
    /// * 0 DLT_STRING("FLDA")
    /// * 1 DLT_UINT(file-id)
    /// * 2 DLT_UINT(packet-num)
    /// * 3 DLT_RAW(bytes)
    /// * 4 DLT_STRING("FLDA")
    fn data_message(timestamp: Option<u32>, args: &[Argument]) -> Option<FtMessage> {
        let id;
        let packet;
        let bytes;

        if let Some(arg) = args.get(1) {
            if let Some(value) = Self::get_number(arg) {
                id = value;
            } else {
                return None;
            }
        } else {
            return None;
        }

        if let Some(arg) = args.get(2) {
            if let Some(value) = Self::get_number(arg) {
                packet = value;
            } else {
                return None;
            }
        } else {
            return None;
        }

        if let Some(arg) = args.get(3) {
            if let Some(value) = Self::get_bytes(arg) {
                bytes = value;
            } else {
                return None;
            }
        } else {
            return None;
        }

        Some(FtMessage::Data(FileData {
            timestamp,
            id,
            packet,
            bytes,
        }))
    }

    /// Returns a DLT-FT end message from a DLT argument list, if any.
    ///
    /// # Expected arguments:
    ///
    /// * 0 DLT_STRING("FLFI")
    /// * 1 DLT_UINT(file-id)
    /// * 2 DLT_STRING("FLFI")
    fn end_message(timestamp: Option<u32>, args: &[Argument]) -> Option<FtMessage> {
        let id;

        if let Some(arg) = args.get(1) {
            if let Some(value) = Self::get_number(arg) {
                id = value;
            } else {
                return None;
            }
        } else {
            return None;
        }

        Some(FtMessage::End(FileEnd { timestamp, id }))
    }

    /// Returns a string value from given argument, if any.
    fn get_string(arg: &Argument) -> Option<String> {
        if let Value::StringVal(value) = &arg.value {
            return Some(value.trim().to_string());
        }

        None
    }

    /// Returns a number value from given argument, if any.
    fn get_number(arg: &Argument) -> Option<u32> {
        if let Value::U32(value) = &arg.value {
            return Some(*value);
        }

        None
    }

    /// Returns a byte value from given argument, if any.
    fn get_bytes(arg: &Argument) -> Option<&Vec<u8>> {
        if let Value::Raw(value) = &arg.value {
            return Some(value);
        }

        None
    }
}
