use crate::{sde::SdeMsg, ByteSource, ReloadInfo, SourceFilter};
use async_stream::stream;
use log::warn;
use parsers::{Error as ParserError, LogMessage, MessageStreamItem, Parser};
use std::marker::PhantomData;
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};
use tokio_stream::Stream;

pub type SdeSender = UnboundedSender<SdeMsg>;
pub type SdeReceiver = UnboundedReceiver<SdeMsg>;

enum Next {
    Read((usize, usize, usize)),
    Sde(Option<SdeMsg>),
}

#[derive(Debug)]
pub struct MessageProducer<T, P, D>
where
    T: LogMessage,
    P: Parser<T>,
    D: ByteSource,
{
    byte_source: D,
    index: usize,
    parser: P,
    filter: Option<SourceFilter>,
    last_seen_ts: Option<u64>,
    _phantom_data: Option<PhantomData<T>>,
    total_loaded: usize,
    total_skipped: usize,
    done: bool,
    rx_sde: Option<SdeReceiver>,
}

impl<T: LogMessage, P: Parser<T>, D: ByteSource> MessageProducer<T, P, D> {
    /// create a new producer by plugging into a byte source
    pub fn new(parser: P, source: D, rx_sde: Option<SdeReceiver>) -> Self {
        MessageProducer {
            byte_source: source,
            index: 0,
            parser,
            filter: None,
            last_seen_ts: None,
            _phantom_data: None,
            total_loaded: 0,
            total_skipped: 0,
            done: false,
            rx_sde,
        }
    }
    /// create a stream of pairs that contain the count of all consumed bytes and the
    /// MessageStreamItem
    pub fn as_stream(&mut self) -> impl Stream<Item = (usize, MessageStreamItem<T>)> + '_ {
        stream! {
            'main: loop {
                if self.done {
                    debug!("done...no next segment");
                    break 'main;
                }
                self.index += 1;
                let (_newly_loaded, mut available, mut skipped_bytes) = 'outer: loop {
                    if let Some(mut rx_sde) = self.rx_sde.take() {
                        'inner: loop {
                            // SDE mode: listening next chunk and possible incoming message for source
                            match select! {
                                msg = rx_sde.recv() => Next::Sde(msg),
                                read = self.do_reload() => Next::Read(read.unwrap_or((0, 0, 0))),
                            } {
                                Next::Read(next) => {
                                    self.rx_sde = Some(rx_sde);
                                    break 'outer next;
                                }
                                Next::Sde(msg) => {
                                    if let Some((msg, tx_response)) = msg {
                                        if tx_response
                                            .send(
                                                self.byte_source
                                                    .income(msg)
                                                    .await
                                                    .map_err(|e| e.to_string()),
                                            )
                                            .is_err()
                                        {
                                            warn!("Fail to send back message from source");
                                        }
                                    } else {
                                        // Means - no more senders; but it isn't an error as soon as implementation of
                                        // source could just do not use a data exchanging
                                        self.rx_sde = None;
                                        // Exiting from inner loop to avoid select! and go to NoSDE mode
                                        break 'inner;
                                    }
                                }
                            }
                        }
                    } else {
                        // NoSDE mode: listening only next chunk
                        break 'outer self.do_reload().await.unwrap_or((0, 0, 0));
                    };
                };
                // 1. buffer loaded? if not, fill buffer with frame data
                // 2. try to parse message from buffer
                // 3a. if message, pop it of the buffer and deliever
                // 3b. else reload into buffer and goto 2
                loop {
                    let current_slice = self.byte_source.current_slice();
                    debug!(
                        "current slice: (len: {}) (total {})",
                        current_slice.len(),
                        self.total_loaded
                    );
                    if available == 0 {
                        trace!("No more bytes available from source");
                        self.done = true;
                        yield (0, MessageStreamItem::Done);
                        break;
                    }
                    // Consumed can't be called from within the for loop for the iterator.
                    let mut total_consumed = 0;

                    //TODO AAZ: Solve this in better way if we need to continue with this approach
                    // Reload can't be called from within the for loop for the iterator.
                    // Reload is called twice in the original code:
                    // Once on Error of type Incomplete
                    let mut need_reload_incomplete = false;
                    // And the other on parse Error
                    let mut need_reload_parse_error = false;

                    // This is the only way currently to get the size hint of the iterator without
                    // invoking the error about Send implementation isn't general enough
                    let (results_iter, results_size_hint) = {
                        let iter = self.parser.parse(self.byte_source.current_slice(), self.last_seen_ts);
                        let len_hint = iter.size_hint().1;
                        (iter, len_hint)
                    };


                    // We can't yield the results from within the loop while we still have
                    // reference to the iterator. This approach saves the values in a vector
                    // temporally and yield them after that one by one.
                    // TODO AAZ: Change the stream output to vector or an iterator
                    let mut results_cache = if let Some(len) = results_size_hint {
                        Vec::with_capacity(len)
                    }else {
                        Vec::new()
                    };

                    for parse_res in results_iter {
                        match parse_res {
                            Ok((consumed, Some(m))) => {
                                let total_used_bytes = consumed + skipped_bytes;
                                debug!(
                                    "Extracted a valid message, consumed {} bytes (total used {} bytes)",
                                    consumed, total_used_bytes
                                );
                                total_consumed += consumed;
                                results_cache.push((total_used_bytes, MessageStreamItem::Item(m)));
                            }
                            Ok((consumed, None)) => {
                                total_consumed += consumed;
                                trace!("None, consumed {} bytes", consumed);
                                let total_used_bytes = consumed + skipped_bytes;
                                results_cache.push((total_used_bytes, MessageStreamItem::Skipped));
                            }
                            Err(ParserError::Incomplete) => {
                                trace!("not enough bytes to parse a message");
                                need_reload_incomplete = true;
                                continue;
                            }
                            Err(ParserError::Eof) => {
                                trace!(
                                    "EOF reached...no more messages (skipped_bytes={})",
                                    skipped_bytes
                                );
                                break 'main;
                            }
                            Err(ParserError::Parse(s)) => {
                                trace!(
                                "No parse possible, try next batch of data ({}), skipped {} more bytes ({} already)",
                                s, available, skipped_bytes
                            );
                                total_consumed += available;
                                // skip all currently available bytes
                                // self.byte_source.consume(available);
                                skipped_bytes += available;
                                need_reload_parse_error = true;
                            }
                        }
                    }

                    for res in results_cache {
                        yield res;
                    }

                    self.byte_source.consume(total_consumed);

                    if need_reload_incomplete {
                        let Some((reloaded, _available_bytes, skipped)) = self.do_reload().await else {break 'main};
                        available += reloaded;
                        skipped_bytes += skipped;
                    }

                    if need_reload_parse_error {
                        available = self.byte_source.len();
                        if let Some((reloaded, _available_bytes, skipped)) = self.do_reload().await {
                            available += reloaded;
                            skipped_bytes += skipped;
                        } else {
                            let unused = skipped_bytes + available;
                            self.done = true;
                            yield (unused, MessageStreamItem::Done);
                        }
                    }
                }
            }
        }
    }

    async fn do_reload(&mut self) -> Option<(usize, usize, usize)> {
        match self.byte_source.reload(self.filter.as_ref()).await {
            Ok(Some(ReloadInfo {
                newly_loaded_bytes,
                available_bytes,
                skipped_bytes,
                last_known_ts,
            })) => {
                self.total_loaded += newly_loaded_bytes;
                self.total_skipped += skipped_bytes;
                if let Some(ts) = last_known_ts {
                    self.last_seen_ts = Some(ts);
                }
                trace!(
                    "did a do_reload, skipped {} bytes, loaded {} more bytes (total loaded and skipped: {})",
                    skipped_bytes, newly_loaded_bytes, self.total_loaded + self.total_skipped
                );
                Some((newly_loaded_bytes, available_bytes, skipped_bytes))
            }
            Ok(None) => {
                trace!("byte_source.reload result was None");
                None
            }
            Err(e) => {
                warn!("Error reloading content: {}", e);
                None
            }
        }
    }
}
