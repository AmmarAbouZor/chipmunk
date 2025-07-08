#[cfg(test)]
mod tests;

pub mod sde;
mod types;

pub use types::{ParseOperationInfos, ProcessResult};

use definitions::*;
use log::warn;

/// Number of bytes to skip on initial parse errors before terminating the session.
const INITIAL_PARSE_ERROR_LIMIT: usize = 1024;

/// Number of bytes to drop when trying to recover from parser errors.
const DROP_STEP: usize = 1;

#[derive(Debug)]
pub struct MessageProducer<P, D>
where
    P: Parser,
    D: ByteSource,
{
    byte_source: D,
    parser: P,
    filter: Option<SourceFilter>,
    last_seen_ts: Option<u64>,
    /// Bytes count on last load call.
    last_loaded_bytes: usize,
    total_produced_items: usize,
}

#[derive(Debug)]
pub enum FetchResult {
    FetchInfo {
        newly_loaded_bytes: usize,
        available_bytes: usize,
        skipped_bytes: usize,
    },
    Error(SourceError),
}

impl<P: Parser, D: ByteSource> MessageProducer<P, D> {
    /// create a new producer by plugging into a byte source
    pub fn new(parser: P, source: D) -> Self {
        MessageProducer {
            byte_source: source,
            parser,
            filter: None,
            last_seen_ts: None,
            last_loaded_bytes: 0,
            total_produced_items: 0,
        }
    }

    /// Loads the next segment of bytes. This method must be cancel safe.
    ///
    /// # Cancel Safety:
    /// This function is cancel safe as long [`ByteSource::load()`] method on used byte source is
    /// safe as well.
    pub async fn fetch_data(&mut self) -> FetchResult {
        match self.byte_source.load(self.filter.as_ref()).await {
            Ok(Some(ReloadInfo {
                newly_loaded_bytes,
                available_bytes,
                skipped_bytes,
                last_known_ts,
            })) => {
                self.last_loaded_bytes = newly_loaded_bytes;
                if let Some(ts) = last_known_ts {
                    self.last_seen_ts = Some(ts);
                }

                trace!(
                    "Load succeeded, skipped {skipped_bytes} bytes, loaded {newly_loaded_bytes}\
                        more bytes. total available: {available_bytes}",
                );

                FetchResult::FetchInfo {
                    newly_loaded_bytes,
                    available_bytes,
                    skipped_bytes,
                }
            }
            Ok(None) => {
                trace!("byte_source.reload result was None");

                FetchResult::FetchInfo {
                    newly_loaded_bytes: 0,
                    available_bytes: self.byte_source.len(),
                    skipped_bytes: 0,
                }
            }
            Err(e) => {
                // In error case we don't need to consider the available bytes.
                warn!("Error reloading content: {}", e);
                FetchResult::Error(e)
            }
        }
    }

    /// Processes the current available data returning the process results
    ///
    /// * `logs_buffer`: Buffer to append the parsed logs into.
    pub fn process_data<B: LogRecordsBuffer>(&mut self, logs_buffer: &mut B) -> ProcessResult {
        let mut dropped_bytes = 0;
        loop {
            // *** Dropped Bytes Check ***
            // TODO: This is temporary solution. We need to inform the user each time we
            // hit the `INITIAL_PARSE_ERROR_LIMIT` and not break the session.
            // We may need the new item `ProcessResult::Dropped(bytes_count)`
            //
            // Return early when initial parse calls fail after consuming one megabyte.
            // This can happen when provided bytes aren't suitable for the select parser.
            // In such case we close the session directly to avoid having unresponsive
            // state while parse is calling on each skipped byte in the source.
            if !self.did_produce_items() && dropped_bytes > INITIAL_PARSE_ERROR_LIMIT {
                let err_msg = "Aborting session due to failing initial parse call";
                warn!("{err_msg}");

                return ProcessResult::Error(ParserError::Unrecoverable(err_msg.into()));
            }

            // *** Parsing ***
            let current_slice = self.byte_source.current_slice();
            debug!("current slice: (len: {})", current_slice.len());

            if current_slice.is_empty() {
                trace!("No more bytes available from source");

                return ProcessResult::None;
            }

            let mut total_consumed = 0;
            let mut parsed_msgs = 0;
            let mut skipped_msgs = 0;

            // we can call consume only after all parse results are collected because of its
            // reference to self.
            match self
                .parser
                .parse(current_slice, self.last_seen_ts)
                .map(|iter| {
                    iter.for_each(|item| match item {
                        (consumed, Some(m)) => {
                            trace!("Extracted a valid message, consumed {consumed} bytes");
                            total_consumed += consumed;
                            parsed_msgs += 1;

                            logs_buffer.append(m)
                        }
                        (consumed, None) => {
                            trace!("Skippped Message, consumed {} bytes", consumed);
                            total_consumed += consumed;
                            skipped_msgs += 1;
                        }
                    })
                }) {
                Ok(()) => {
                    self.total_produced_items += parsed_msgs;
                    self.byte_source.consume(total_consumed);
                    let parse_info =
                        ParseOperationInfos::new(total_consumed, parsed_msgs, skipped_msgs);
                    return ProcessResult::Parsed(parse_info);
                }
                Err(err @ ParserError::Incomplete) => {
                    debug!("not enough bytes to parse a message. Load more data");

                    // Start dropping bytes if last load call didn't provided
                    // any new bytes.
                    // In this case the parser is returning an incomplete error
                    // while the byte source is full with bytes.
                    if self.last_loaded_bytes == 0 {
                        trace!("No parse possible, skip one byte and retry. Error: {err}");

                        self.byte_source.consume(DROP_STEP);
                        dropped_bytes += DROP_STEP;
                        // Retry to parse after drop.
                        if !self.byte_source.is_empty() {
                            continue;
                        }
                    } else {
                        return ProcessResult::Error(err);
                    }
                }
                Err(err @ ParserError::Parse(..)) => {
                    debug!("No parse possible, skip one byte and retry. Error: {err}");

                    self.byte_source.consume(DROP_STEP);
                    dropped_bytes += DROP_STEP;
                    // Retry to parse after drop.
                    if !self.byte_source.is_empty() {
                        continue;
                    }

                    return ProcessResult::Error(err);
                }
                Err(err @ ParserError::Eof) => {
                    debug!("End of File reached");

                    return ProcessResult::Error(err);
                }
                Err(err @ ParserError::Unrecoverable(..)) => {
                    error!("Parsing failed: Error {err}");

                    return ProcessResult::Error(err);
                }
            }
        }
    }

    /// Checks if the producer have already produced any parsed items in the current session.
    #[inline]
    fn did_produce_items(&self) -> bool {
        self.total_produced_items > 0
    }

    /// Append incoming (SDE) Source-Data-Exchange to the underline byte source data.
    pub async fn sde_income(
        &mut self,
        msg: stypes::SdeRequest,
    ) -> Result<stypes::SdeResponse, SourceError> {
        self.byte_source.income(msg).await
    }
}
