use crate::{
    operations::{OperationAPI, OperationResult},
    state::SessionStateAPI,
    tail,
};
use definitions::*;
use log::warn;
use processor::producer::{FetchResult, MessageProducer, ProcessResult, sde::*};
use std::time::{Duration, Instant};
use tokio::{select, sync::mpsc::Receiver, time::timeout};

pub const FLUSH_TIMEOUT_IN_MS: u64 = 500;

/// A buffer for accumulating log data before writing to a session file.
///
/// This buffer is linked to a session file via the `SessionStateAPI`.
/// It accumulates updates and sends them when `flush()` is called.
///
/// It uses two internal stores to manage data:
/// - `buffer` accumulates textual log entries.
/// - `attachments` stores associated `Attachment` objects. These are sent
///   only after the text buffer is flushed to ensure synchronization.
pub struct LogsBuffer {
    /// Communication channel to the session file.
    state: SessionStateAPI,

    /// Text buffer for log messages. Since session files are text-based,
    /// the buffered data is stored as a `String`.
    text_buffer: String,

    /// Buffer for received attachments. These are queued and sent only after
    /// the buffered log messages have been flushed, preserving the logical
    /// order between messages and attachments.
    attachments: Vec<Attachment>,

    /// Unique identifier for the data source. This is used on the client side
    /// to visually group or distinguish data streams.
    id: u16,
}

impl LogsBuffer {
    pub fn new(state: SessionStateAPI, id: u16) -> Self {
        Self {
            state,
            text_buffer: String::new(),
            attachments: Vec::new(),
            id,
        }
    }
}

impl LogRecordsBuffer for LogsBuffer {
    fn append(&mut self, record: LogRecordOutput<'_>) {
        match record {
            LogRecordOutput::Raw(inner) => {
                // TODO: Needs to be optimized. Also this use-case doesn't seem normal, should be some logs
                // because during observe we do not expect raw data
                self.text_buffer.push_str(
                    &inner
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<String>(),
                );
                self.text_buffer.push('\n');
            }
            LogRecordOutput::Message(msg) => {
                self.text_buffer.push_str(&msg);
                self.text_buffer.push('\n');
            }
            LogRecordOutput::Columns(inner) => {
                let mut items = inner.into_iter();
                if let Some(first_item) = items.next() {
                    self.text_buffer.push_str(&first_item);
                    for item in items {
                        self.text_buffer.push(definitions::COLUMN_SENTINAL);
                        self.text_buffer.push_str(&item);
                    }
                }
                self.text_buffer.push('\n');
            }
            LogRecordOutput::Multiple(inner) => {
                for rec in inner {
                    self.append(rec);
                }
            }
            LogRecordOutput::Attachment(inner) => {
                self.attachments.push(inner);
            }
        }
    }

    async fn flush(&mut self) -> Result<(), stypes::NativeError> {
        if !self.text_buffer.is_empty() {
            // Creates an owned string from current buffer then clean the current. This operation
            // produces one mem_copy command for the needed bytes only while preserving
            // the capacity of the intermediate buffer.
            // Rust doesn't provide safe way to move bytes between strings without replacing
            // the whole string, forcing us to allocate the full capacity of the buffer on each
            // iteration (which could backfire in the internal buffer gets too long in one of the
            // iterations).
            let msgs = String::from(&self.text_buffer);
            self.text_buffer.clear();
            self.state
                .write_session_file(self.get_source_id(), msgs)
                .await?;
        }
        for attachment in self.attachments.drain(..) {
            // TODO: send with 1 call
            self.state.add_attachment(attachment)?;
        }
        Ok(())
    }

    fn get_source_id(&self) -> u16 {
        self.id
    }
}

#[derive(Debug)]
enum Next {
    FetchResult(FetchResult),
    Sde(SdeMsg),
    Timeout,
}

pub async fn run_producer<P: Parser, S: ByteSource, B: LogRecordsBuffer>(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    mut producer: MessageProducer<P, S>,
    logs_buffer: &mut B,
    mut rx_tail: Option<Receiver<Result<(), tail::Error>>>,
    mut rx_sde: Option<SdeReceiver>,
) -> OperationResult<()> {
    use log::debug;
    state.set_session_file(None).await?;
    operation_api.processing();
    let cancel = operation_api.cancellation_token();
    let cancel_on_tail = cancel.clone();
    let started = Instant::now();

    while let Some(next) = select! {
        next_from_stream = async {
            match timeout(Duration::from_millis(FLUSH_TIMEOUT_IN_MS), producer.fetch_data()).await {
                Ok(res) => Some(Next::FetchResult(res)),
                Err(_) => Some(Next::Timeout),
            }
        } => next_from_stream,
        Some(sde_msg) = async {
            if let Some(rx_sde) = rx_sde.as_mut() {
                rx_sde.recv().await
            } else {
                None
            }
        } => Some(Next::Sde(sde_msg)),
        _ = cancel.cancelled() => None,
    } {
        match next {
            Next::FetchResult(FetchResult::FetchInfo {
                newly_loaded_bytes: _,
                available_bytes,
                skipped_bytes: _,
            }) => {
                // No available_bytes => Wait for tailing.
                if available_bytes == 0 {
                    warn!(
                        "observe, message stream has entered tailing in {} ms",
                        started.elapsed().as_millis()
                    );
                    if !state.is_closing() {
                        state.flush_session_file().await?;
                    }
                    if let Some(mut rx_tail) = rx_tail.take() {
                        if select! {
                            next_from_stream = rx_tail.recv() => {
                                if let Some(result) = next_from_stream {
                                    result.is_err()
                                } else {
                                    true
                                }
                            },
                            _ = cancel_on_tail.cancelled() => true,
                        } {
                            break;
                        }
                    } else {
                        break;
                    }
                    continue;
                }
                // Run process data
                match producer.process_data(logs_buffer) {
                    ProcessResult::Parsed(parse_operation_infos) => {
                        // TODO: Report results to users

                        if parse_operation_infos.parsed_msgs > 0 {
                            if let Err(err) = logs_buffer.flush().await {
                                log::error!("Fail to write data into writer: {err}");
                            }
                        }
                    }
                    ProcessResult::None => {
                        // No data available => Try to load more & tailing.
                    }
                    //TODO: Errors must be visible to users.
                    ProcessResult::Error(parser_error) => match parser_error {
                        ParserError::Unrecoverable(err) => {
                            eprintln!("Ending session due to unrecoverable error {err}");
                            break;
                        }
                        ParserError::Parse(err) => {
                            eprintln!("Paring Error: {err}");
                            // Don't stop on parse errors.
                            // However, we need to show them to users.
                            continue;
                        }
                        ParserError::Incomplete => {
                            // Load more bytes the next round.
                            continue;
                        }
                        ParserError::Eof => {
                            break;
                        }
                    },
                }
            }
            Next::Timeout => {
                if !state.is_closing() {
                    state.flush_session_file().await?;
                }
            }
            Next::FetchResult(FetchResult::Error(err)) => {
                // TODO: Also here we need to report errors to users.
                eprintln!("Ending session due to source error {err}");
                break;
            }
            Next::Sde((msg, tx_response)) => {
                let sde_res = producer.sde_income(msg).await.map_err(|e| e.to_string());
                if tx_response.send(sde_res).is_err() {
                    warn!("Fail to send back message from source");
                }
            }
        }
    }

    state.flush_session_file().await?;
    state.file_read().await?;
    warn!(
        "observe, message stream is done in {} ms",
        started.elapsed().as_millis()
    );

    debug!("listen done");
    Ok(None)
}
