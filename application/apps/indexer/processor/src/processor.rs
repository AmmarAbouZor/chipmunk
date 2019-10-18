// Copyright (c) 2019 E.S.R.Labs. All rights reserved.
//
// NOTICE:  All information contained herein is, and remains
// the property of E.S.R.Labs and its suppliers, if any.
// The intellectual and technical concepts contained herein are
// proprietary to E.S.R.Labs and its suppliers and may be covered
// by German and Foreign Patents, patents in process, and are protected
// by trade secret or copyright law.
// Dissemination of this information or reproduction of this material
// is strictly forbidden unless prior written permission is obtained
// from E.S.R.Labs.

use crate::parse;
use failure::{err_msg, Error};
use indexer_base::chunks::{Chunk, ChunkFactory};
use indexer_base::config::IndexingConfig;
use indexer_base::error_reporter::*;
use indexer_base::progress::*;
use indexer_base::utils;
use parse::detect_timestamp_in_string;
use std::sync::mpsc::{self, TryRecvError};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::time::Instant;

const REPORT_PROGRESS_LINE_BLOCK: usize = 1_000_000;

pub fn create_index_and_mapping(
    config: IndexingConfig,
    parse_timestamps: bool,
    source_file_size: Option<usize>,
    update_channel: Option<mpsc::Sender<IndexingProgress<Chunk>>>,
    shutdown_receiver: Option<mpsc::Receiver<()>>,
) -> Result<Vec<Chunk>, Error> {
    let initial_line_nr = match utils::next_line_nr(config.out_path) {
        Some(nr) => nr,
        None => {
            report_error(format!(
                "could not determine last line number of {:?}",
                config.out_path
            ));
            std::process::exit(2)
        }
    };
    index_file(
        config,
        initial_line_nr,
        parse_timestamps,
        source_file_size,
        update_channel,
        shutdown_receiver,
    )
}

pub fn index_file(
    config: IndexingConfig,
    initial_line_nr: usize,
    timestamps: bool,
    source_file_size: Option<usize>,
    update_channel: Option<mpsc::Sender<IndexingProgress<Chunk>>>,
    shutdown_receiver: Option<mpsc::Receiver<()>>,
) -> Result<Vec<Chunk>, Error> {
    let start = Instant::now();
    let (out_file, current_out_file_size) =
        utils::get_out_file_and_size(config.append, &config.out_path)?;

    let mut chunks = vec![];
    let mut chunk_factory =
        ChunkFactory::new(config.chunk_size, config.to_stdout, current_out_file_size);

    let mut reader = BufReader::new(config.in_file);
    let mut line_nr = initial_line_nr;
    let mut buf_writer = BufWriter::with_capacity(10 * 1024 * 1024, out_file);

    let mut buf = vec![];
    let mut processed_bytes = 0usize;
    let mut stopped = false;
    while let Ok(len) = reader.read_until(b'\n', &mut buf) {
        if stopped {
            info!("we where stopped in indexer",);
            break;
        };
        let s = unsafe { std::str::from_utf8_unchecked(&buf) };
        let trimmed_line = s.trim_matches(utils::is_newline);
        let trimmed_len = trimmed_line.len();
        let had_newline = trimmed_len != len;
        processed_bytes += len;
        if len == 0 {
            // no more content
            break;
        };
        let additional_bytes: usize;
        // only use non-empty lines, others will be dropped
        if trimmed_len != 0 {
            if timestamps {
                let ts = match detect_timestamp_in_string(trimmed_line, None) {
                    Ok((time, _, _)) => time,
                    Err(_) => 0,
                };
                if had_newline {
                    writeln!(
                        buf_writer,
                        "{}{}{}{}{}{}{}{}{}",
                        trimmed_line,
                        utils::PLUGIN_ID_SENTINAL,
                        config.tag,
                        utils::PLUGIN_ID_SENTINAL,
                        utils::ROW_NUMBER_SENTINAL,
                        line_nr,
                        utils::ROW_NUMBER_SENTINAL,
                        ts,
                        utils::ROW_NUMBER_SENTINAL,
                    )?;
                } else {
                    write!(
                        buf_writer,
                        "{}{}{}{}{}{}{}{}{}",
                        trimmed_line,
                        utils::PLUGIN_ID_SENTINAL,
                        config.tag,
                        utils::PLUGIN_ID_SENTINAL,
                        utils::ROW_NUMBER_SENTINAL,
                        line_nr,
                        utils::ROW_NUMBER_SENTINAL,
                        ts,
                        utils::ROW_NUMBER_SENTINAL,
                    )?;
                }
                additional_bytes = utils::extended_line_length(
                    trimmed_len,
                    config.tag.len(),
                    line_nr,
                    had_newline,
                ) + utils::linenr_length(ts as usize)
                    + 1;
            } else {
                if had_newline {
                    writeln!(
                        buf_writer,
                        "{}{}{}{}{}{}{}",
                        trimmed_line,
                        utils::PLUGIN_ID_SENTINAL,
                        config.tag,
                        utils::PLUGIN_ID_SENTINAL,
                        utils::ROW_NUMBER_SENTINAL,
                        line_nr,
                        utils::ROW_NUMBER_SENTINAL,
                    )?;
                } else {
                    write!(
                        buf_writer,
                        "{}{}{}{}{}{}{}",
                        trimmed_line,
                        utils::PLUGIN_ID_SENTINAL,
                        config.tag,
                        utils::PLUGIN_ID_SENTINAL,
                        utils::ROW_NUMBER_SENTINAL,
                        line_nr,
                        utils::ROW_NUMBER_SENTINAL,
                    )?;
                }
                additional_bytes = utils::extended_line_length(
                    trimmed_len,
                    config.tag.len(),
                    line_nr,
                    had_newline,
                );
            }
            line_nr += 1;

            match chunk_factory.create_chunk_if_needed(line_nr, additional_bytes) {
                Some(chunk) => {
                    // check if stop was requested
                    if let Some(rx) = shutdown_receiver.as_ref() {
                        match rx.try_recv() {
                            // Shutdown if we have received a command or if there is
                            // nothing to send it.
                            Ok(_) | Err(TryRecvError::Disconnected) => {
                                info!("shutdown received in indexer",);
                                stopped = true // stop
                            }
                            // No shutdown command, continue
                            Err(TryRecvError::Empty) => (),
                        }
                    };
                    update_channel.as_ref().map(|c| {
                        c.send(IndexingProgress::GotItem {
                            item: chunk.clone(),
                        })
                    });
                    chunks.push(chunk);
                    buf_writer.flush()?;
                    false
                }
                None => false,
            };

            if let Some(file_size) = source_file_size {
                utils::report_progress(
                    line_nr,
                    chunk_factory.get_current_byte_index(),
                    processed_bytes,
                    file_size,
                    REPORT_PROGRESS_LINE_BLOCK,
                );
                if line_nr % REPORT_PROGRESS_LINE_BLOCK == 0 {
                    update_channel.as_ref().map(|c| {
                        c.send(IndexingProgress::Progress {
                            ticks: (processed_bytes, file_size),
                        })
                    });
                }
            }
        }
        buf = vec![];
    }
    if stopped {
        if let Some(tx) = update_channel {
            debug!("sending IndexingProgress::Stopped");
            tx.send(IndexingProgress::Stopped)?;
        }
        Ok(chunks)
    } else {
        buf_writer.flush()?;
        if let Some(chunk) = chunk_factory.create_last_chunk(line_nr, chunks.is_empty()) {
            update_channel.as_ref().map(|c| {
                c.send(IndexingProgress::GotItem {
                    item: chunk.clone(),
                })
            });
            chunks.push(chunk);
        }
        match chunks.last() {
            Some(last_chunk) => {
                let last_expected_byte_index =
                    fs::metadata(config.out_path).map(|md| md.len() as usize)?;
                if last_expected_byte_index != last_chunk.b.1 {
                    return Err(err_msg(format!(
                        "error in computation! last byte in chunks is {} but should be {}",
                        last_chunk.b.1, last_expected_byte_index
                    )));
                }
            }
            None => report_warning("output was empty"),
        }
        let elapsed = start.elapsed();
        let ms = elapsed.as_millis();
        info!("done, created {} chunks in {} ms", chunks.len(), ms);
        if let Some(tx) = update_channel {
            trace!("sending IndexingProgress::Finished");
            tx.send(IndexingProgress::Finished)?;
        }
        Ok(chunks)
    }
}
