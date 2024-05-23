use std::{path::Path, time::Instant};

use async_stream::stream;
use futures::Stream;
use parsers::MessageStreamItem;

use crate::PluginParseMessage;

use super::WasmProducer;

pub struct WasmProducerWrapper {
    producer: WasmProducer,
    start: Option<Instant>,
}

impl WasmProducerWrapper {
    pub async fn create(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let producer = WasmProducer::create(file_path).await?;

        Ok(Self {
            producer,
            start: None,
        })
    }

    /// create a stream of pairs that contain the count of all consumed bytes and the
    /// MessageStreamItem
    pub fn as_stream_wasm(
        &mut self,
    ) -> impl Stream<Item = (usize, MessageStreamItem<PluginParseMessage>)> + '_ {
        assert!(
            self.start.is_none(),
            "as_stream_wasm() must be called once only"
        );
        self.start = Some(Instant::now());
        stream! {
            while let Some(item) = self.read_next_segment().await {
                yield item;
            }
        }
    }

    async fn read_next_segment(
        &mut self,
    ) -> Option<(usize, MessageStreamItem<PluginParseMessage>)> {
        let Some(parse_result) = self.producer.read_next().await else {
            println!(
                "\x1b[93mmessage producer took : {:?}\x1b[0m",
                self.start.unwrap().elapsed()
            );
            return None;
        };

        match parse_result {
            // used_bytes doesn't match the native code but it isn't used in `run_producer()`
            // anyway and can be skipped in prototyping.
            Ok((used_bytes, Some(m))) => {
                return Some((used_bytes, MessageStreamItem::Item(m)));
            }
            err => {
                unreachable!("Only happy path is implemented. err: {err:?}");
            }
        }
    }
}
