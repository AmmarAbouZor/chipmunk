use std::{
    env,
    path::{Path, PathBuf},
};

use crate::{
    events::{NativeError, NativeErrorKind},
    operations::{OperationAPI, OperationResult},
    progress::Severity,
    state::SessionStateAPI,
    tail,
};
use log::trace;
use parsers::{
    dlt::{fmt::FormatOptions, DltParser},
    someip::SomeipParser,
    text::StringTokenizer,
    LogMessage, MessageStreamItem, ParseYield, Parser,
};
use plugin_host::WasmProducerWrapper;
use sources::{
    factory::ParserType,
    producer::{MessageProducer, SdeReceiver},
    ByteSource,
};
use tokio::{
    select,
    sync::mpsc::Receiver,
    time::{timeout, Duration},
};
use tokio_stream::StreamExt;

enum Next<T: LogMessage> {
    Item(MessageStreamItem<T>),
    Timeout,
    Waiting,
}

pub mod concat;
pub mod file;
pub mod stream;

pub const FLUSH_TIMEOUT_IN_MS: u128 = 500;

pub const USE_WASM_DLT_ENV: &str = "WASM_PARSE";
pub const USE_WASM_ITNERFACE_DLT_ENV: &str = "WASM_PARSE_INTER";
pub const USE_WASM_DLT_ENV2: &str = "WASM_PARSE2";

pub async fn run_source<S: ByteSource>(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    source: S,
    source_id: u16,
    parser: &ParserType,
    rx_sde: Option<SdeReceiver>,
    rx_tail: Option<Receiver<Result<(), tail::Error>>>,
) -> OperationResult<()> {
    match parser {
        ParserType::SomeIp(settings) => {
            let someip_parser = match &settings.fibex_file_paths {
                Some(paths) => {
                    SomeipParser::from_fibex_files(paths.iter().map(PathBuf::from).collect())
                }
                None => SomeipParser::new(),
            };
            let producer = MessageProducer::new(someip_parser, source, rx_sde);
            run_producer(operation_api, state, source_id, producer, rx_tail).await
        }
        ParserType::Text => {
            let producer = MessageProducer::new(StringTokenizer {}, source, rx_sde);
            run_producer(operation_api, state, source_id, producer, rx_tail).await
        }
        ParserType::Dlt(settings) => {
            match (
                env::var(USE_WASM_DLT_ENV),
                env::var(USE_WASM_DLT_ENV2),
                env::var(USE_WASM_ITNERFACE_DLT_ENV),
            ) {
                (Ok(var), _, _) => {
                    println!("------------------------------------------------------");
                    println!("-------------    WASM parser used    -----------------");
                    println!("------------------------------------------------------");
                    let method = match var.as_str() {
                        "vec" => plugin_host::ParseMethod::ReturnVec,
                        "res_s" => plugin_host::ParseMethod::ResSingle,
                        "res_r" => plugin_host::ParseMethod::ResRange,
                        invalid => unreachable!("Invalid WASM ENV var: {invalid}"),
                    };

                    println!(
                        "Parse Mehtod is currently always: {}",
                        plugin_host::ParseMethod::ResSingle
                    );
                    // println!("Parse Mehtod: {method}");

                    let dummy_path = PathBuf::from(".");
                    //TODO: Add new error type for the plugins
                    let wasm_parser = plugin_host::WasmParser::create(dummy_path, method)
                        .await
                        .map_err(|err| {
                            dbg!(&err);
                            NativeError {
                                kind: NativeErrorKind::Io,
                                severity: Severity::ERROR,
                                message: Some(err.to_string()),
                            }
                        })?;
                    // println!("Wasm Parser Created");

                    let producer = MessageProducer::new(wasm_parser, source, rx_sde);
                    // println!("Producer Created");
                    run_producer(operation_api, state, source_id, producer, rx_tail).await
                }
                (_, Ok(_), _) => {
                    println!("------------------------------------------------------");
                    println!("-------------    WASM parser2 used    -----------------");
                    println!("------------------------------------------------------");
                    //TODO: Add new error type for the plugins
                    let wasm_parser = plugin_host::WasmParser2::create().await.map_err(|err| {
                        dbg!(&err);
                        NativeError {
                            kind: NativeErrorKind::Io,
                            severity: Severity::ERROR,
                            message: Some(err.to_string()),
                        }
                    })?;
                    // println!("Wasm Parser Created");

                    let producer = MessageProducer::new(wasm_parser, source, rx_sde);
                    // println!("Producer Created");
                    run_producer(operation_api, state, source_id, producer, rx_tail).await
                }
                (_, _, Ok(var)) => {
                    println!("------------------------------------------------------");
                    println!("----------  WASM parser itnerface used   -------------");
                    println!("------------------------------------------------------");
                    let method = match var.as_str() {
                        "vec" => plugin_host::ParseMethod::ReturnVec,
                        "res_s" => plugin_host::ParseMethod::ResSingle,
                        "res_r" => plugin_host::ParseMethod::ResRange,
                        invalid => unreachable!("Invalid WASM ENV var: {invalid}"),
                    };

                    println!(
                        "Parse Mehtod is currently always: {}",
                        plugin_host::ParseMethod::ResSingle
                    );
                    // println!("Parse Mehtod: {method}");

                    let dummy_path = PathBuf::from(".");
                    //TODO: Add new error type for the plugins
                    let wasm_parser = plugin_host::WasmParserInter::create(dummy_path, method)
                        .await
                        .map_err(|err| {
                            dbg!(&err);
                            NativeError {
                                kind: NativeErrorKind::Io,
                                severity: Severity::ERROR,
                                message: Some(err.to_string()),
                            }
                        })?;
                    // println!("Wasm Parser Created");

                    let producer = MessageProducer::new(wasm_parser, source, rx_sde);
                    // println!("Producer Created");
                    run_producer(operation_api, state, source_id, producer, rx_tail).await
                }
                _ => {
                    println!("------------------------------------------------------");
                    println!("------------    NATIVE parser used    ----------------");
                    println!("------------------------------------------------------");
                    let fmt_options = Some(FormatOptions::from(settings.tz.as_ref()));
                    let dlt_parser = DltParser::new(
                        settings.filter_config.as_ref().map(|f| f.into()),
                        settings.fibex_metadata.as_ref(),
                        fmt_options.as_ref(),
                        settings.with_storage_header,
                    );
                    let producer = MessageProducer::new(dlt_parser, source, rx_sde);
                    run_producer(operation_api, state, source_id, producer, rx_tail).await
                }
            }
        }
    }
}

async fn run_producer<T: LogMessage, P: Parser<T>, S: ByteSource>(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    source_id: u16,
    mut producer: MessageProducer<T, P, S>,
    mut rx_tail: Option<Receiver<Result<(), tail::Error>>>,
) -> OperationResult<()> {
    use log::debug;
    state.set_session_file(None).await?;
    operation_api.processing();
    let cancel = operation_api.cancellation_token();
    let stream = producer.as_stream();
    futures::pin_mut!(stream);
    let cancel_on_tail = cancel.clone();
    while let Some(next) = select! {
        next_from_stream = async {
            match timeout(Duration::from_millis(FLUSH_TIMEOUT_IN_MS as u64), stream.next()).await {
                Ok(item) => {
                    if let Some((_, item)) = item {
                        Some(Next::Item(item))
                    } else {
                        Some(Next::Waiting)
                    }
                },
                Err(_) => Some(Next::Timeout),
            }
        } => next_from_stream,
        _ = cancel.cancelled() => None,
    } {
        match next {
            Next::Item(item) => {
                match item {
                    MessageStreamItem::Item(ParseYield::Message(item)) => {
                        // println!("Message: {item}");
                        state
                            .write_session_file(source_id, format!("{item}\n"))
                            .await?;
                    }
                    MessageStreamItem::Item(ParseYield::MessageAndAttachment((
                        item,
                        attachment,
                    ))) => {
                        // println!("Message and attachment: {item}, {attachment:?}");
                        state
                            .write_session_file(source_id, format!("{item}\n"))
                            .await?;
                        state.add_attachment(attachment)?;
                    }
                    MessageStreamItem::Item(ParseYield::Attachment(attachment)) => {
                        // println!("Attachment: {attachment:?}");
                        state.add_attachment(attachment)?;
                    }
                    MessageStreamItem::Done => {
                        trace!("observe, message stream is done");
                        // println!("observe, message stream is done");
                        state.flush_session_file().await?;
                        state.file_read().await?;
                    }
                    // MessageStreamItem::FileRead => {
                    //     state.file_read().await?;
                    // }
                    MessageStreamItem::Skipped => {
                        // println!("observe: skipped a message");
                        trace!("observe: skipped a message");
                    }
                    MessageStreamItem::Incomplete => {
                        // println!("observe: incomplete message");
                        trace!("observe: incomplete message");
                    }
                    MessageStreamItem::Empty => {
                        // println!("observe: empty message");
                        trace!("observe: empty message");
                    }
                }
            }
            Next::Timeout => {
                if !state.is_closing() {
                    state.flush_session_file().await?;
                }
            }
            Next::Waiting => {
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
            }
        }
    }
    debug!("listen done");
    Ok(None)
}

pub async fn run_source_wasm(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    file_path: impl AsRef<Path>,
    source_id: u16,
    rx_tail: Option<Receiver<Result<(), tail::Error>>>,
) -> OperationResult<()> {
    run_producer_wasm(operation_api, state, source_id, file_path, rx_tail).await
}

async fn run_producer_wasm(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    source_id: u16,
    file_path: impl AsRef<Path>,
    mut rx_tail: Option<Receiver<Result<(), tail::Error>>>,
) -> OperationResult<()> {
    let mut producer = WasmProducerWrapper::create(file_path).await.unwrap();

    // Code Copied from run_producer() without any change.
    use log::debug;
    state.set_session_file(None).await?;
    operation_api.processing();
    let cancel = operation_api.cancellation_token();
    let stream = producer.as_stream_wasm();
    futures::pin_mut!(stream);
    let cancel_on_tail = cancel.clone();
    while let Some(next) = select! {
        next_from_stream = async {
            match timeout(Duration::from_millis(FLUSH_TIMEOUT_IN_MS as u64), stream.next()).await {
                Ok(item) => {
                    if let Some((_, item)) = item {
                        Some(Next::Item(item))
                    } else {
                        Some(Next::Waiting)
                    }
                },
                Err(_) => Some(Next::Timeout),
            }
        } => next_from_stream,
        _ = cancel.cancelled() => None,
    } {
        match next {
            Next::Item(item) => {
                match item {
                    MessageStreamItem::Item(ParseYield::Message(item)) => {
                        // println!("Message: {item}");
                        state
                            .write_session_file(source_id, format!("{item}\n"))
                            .await?;
                    }
                    MessageStreamItem::Item(ParseYield::MessageAndAttachment((
                        item,
                        attachment,
                    ))) => {
                        // println!("Message and attachment: {item}, {attachment:?}");
                        state
                            .write_session_file(source_id, format!("{item}\n"))
                            .await?;
                        state.add_attachment(attachment)?;
                    }
                    MessageStreamItem::Item(ParseYield::Attachment(attachment)) => {
                        // println!("Attachment: {attachment:?}");
                        state.add_attachment(attachment)?;
                    }
                    MessageStreamItem::Done => {
                        trace!("observe, message stream is done");
                        // println!("observe, message stream is done");
                        state.flush_session_file().await?;
                        state.file_read().await?;
                    }
                    // MessageStreamItem::FileRead => {
                    //     state.file_read().await?;
                    // }
                    MessageStreamItem::Skipped => {
                        // println!("observe: skipped a message");
                        trace!("observe: skipped a message");
                    }
                    MessageStreamItem::Incomplete => {
                        // println!("observe: incomplete message");
                        trace!("observe: incomplete message");
                    }
                    MessageStreamItem::Empty => {
                        // println!("observe: empty message");
                        trace!("observe: empty message");
                    }
                }
            }
            Next::Timeout => {
                if !state.is_closing() {
                    state.flush_session_file().await?;
                }
            }
            Next::Waiting => {
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
            }
        }
    }
    debug!("listen done");
    Ok(None)
}
