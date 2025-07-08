use definitions::ParserError;

/// The results of calling process on producer.
#[derive(Debug)]
pub enum ProcessResult {
    /// Items has been successfully processed and added to logs buffer.
    Parsed(ParseOperationInfos),
    /// Paring failed.
    Error(ParserError),
    /// No data is available to process.
    None,
}

/// Represents the result of a single process step.
///
/// This structure is used to report the outcome of log parsing from `MessageProducer`
/// to its controlling logic.
#[derive(Debug, Clone)]
pub struct ParseOperationInfos {
    /// Number of bytes successfully consumed from the input buffer.
    pub consumed: usize,
    /// Number of messages that were parsed and forwarded.
    pub parsed_msgs: usize,
    /// Number of messages that were skipped.
    pub skipped_msgs: usize,
}

impl ParseOperationInfos {
    pub fn new(consumed: usize, parsed_msgs: usize, skipped_msgs: usize) -> Self {
        Self {
            consumed,
            parsed_msgs,
            skipped_msgs,
        }
    }
}
