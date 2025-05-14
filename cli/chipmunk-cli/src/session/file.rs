//! Provides methods for running a session with a file as the input source.

use tokio_util::sync::CancellationToken;

use parsers::{LogMessage, Parser};
use sources::{ByteSource, producer::MessageProducer};

use super::format::MessageWriter;

/// Message interval to print output status to stdout while parsing.
const UPDATE_MESSAGE_INTERVAL: usize = 5000;

/// Runs a parsing session considering that the parsing will run quickly
/// since the input is inside a file.
///
/// * `parser`: Parser instance to be used for parsing the bytes in the session.
/// * `bytesource`: Byte source instance to deliver the bytes in the session.
/// * `output_path`: The path for the output file path.
/// * `msg_writer`: The formatter and writer for messages in the session.
/// * `cancel_token`: CancellationToken.
pub async fn run_session<T, P, D, W>(
    parser: P,
    bytesource: D,
    mut msg_writer: W,
    cancel_token: CancellationToken,
) -> anyhow::Result<()>
where
    T: LogMessage,
    P: Parser<T>,
    D: ByteSource,
    W: MessageWriter,
{
    let mut producer = MessageProducer::new(parser, bytesource);

    let mut msg_count = 0;
    let mut skipped_count = 0;
    let mut empty_count = 0;
    let mut incomplete_count = 0;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                msg_writer.flush().await?;
                super::write_summary(msg_count, skipped_count, empty_count, incomplete_count);

                return Ok(());
            },
            Some(items) = producer.read_next_segment() => {
                for (_, item) in items {
                    match item {
                        parsers::MessageStreamItem::Item(parse_yield) => {
                            let msg = match parse_yield {
                                parsers::ParseYield::Message(msg) => msg,
                                parsers::ParseYield::Attachment(_attachment) => {
                                    // attachment are postponed for now.
                                    continue;
                                }
                                parsers::ParseYield::MessageAndAttachment((msg, _attachment)) => msg,
                            };
                            msg_writer.write_msg(msg)?;

                            msg_count += 1;
                            if msg_count % UPDATE_MESSAGE_INTERVAL == 0 {
                                println!("Processing... {msg_count} messages have been written to file.");
                            }
                        }
                        parsers::MessageStreamItem::Skipped => skipped_count += 1,
                        parsers::MessageStreamItem::Incomplete => incomplete_count += 1,
                        parsers::MessageStreamItem::Empty => empty_count += 1,
                        parsers::MessageStreamItem::Done => {
                            println!("Parsing Done");
                            super::write_summary(msg_count, skipped_count, empty_count, incomplete_count);

                            return Ok(());
                        }
                    }
                }
            }

        }
    }
}
