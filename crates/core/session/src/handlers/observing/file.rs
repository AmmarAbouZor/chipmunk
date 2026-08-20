use crate::{
    operations::{OperationAPI, OperationResult},
    state::{LinkOutcome, SessionStateAPI},
    tail,
};
use sources::{
    ByteSource,
    binary::{
        pcap::{legacy::PcapLegacyByteSource, ng::PcapngByteSource},
        raw::BinaryByteSource,
    },
};
use std::{fs::File, path::Path};
use tokio::{join, select, sync::mpsc::channel};

pub async fn observe_file(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    uuid: &str,
    file_format: &stypes::FileFormat,
    filename: &Path,
    parser: &stypes::ParserType,
) -> OperationResult<()> {
    let source_id = state.add_source(uuid).await?;
    match file_format {
        stypes::FileFormat::Binary => {
            let source = BinaryByteSource::new(input_file(filename)?);
            produce_from_file(operation_api, state, source, source_id, parser, filename).await
        }
        stypes::FileFormat::PcapLegacy => {
            let source = PcapLegacyByteSource::new(input_file(filename)?)?;
            produce_from_file(operation_api, state, source, source_id, parser, filename).await
        }
        stypes::FileFormat::PcapNG => {
            let source = PcapngByteSource::new(input_file(filename)?)?;
            produce_from_file(operation_api, state, source, source_id, parser, filename).await
        }
        stypes::FileFormat::Text => {
            // We need to count for cases where parsers other than text parser
            // (like plugins) are expected to have text files sources.
            if !matches!(parser, stypes::ParserType::Text(())) {
                let source = BinaryByteSource::new(input_file(filename)?);
                return produce_from_file(
                    operation_api,
                    state,
                    source,
                    source_id,
                    parser,
                    filename,
                )
                .await;
            }

            match state
                .link_session_file(filename.to_path_buf(), source_id)
                .await?
            {
                LinkOutcome::Linked => {
                    tail_linked_file(operation_api, state, source_id, filename).await
                }
                // The file content cannot be read as UTF-8, so it is transcoded into a session
                // file of its own instead of being served from the original bytes.
                LinkOutcome::NotUtf8 => {
                    let source = BinaryByteSource::new(input_file(filename)?);
                    produce_from_file(operation_api, state, source, source_id, parser, filename)
                        .await
                }
            }
        }
    }
}

/// Feeds the session from the file through the parser, following the file while it grows.
async fn produce_from_file<S: ByteSource>(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    source: S,
    source_id: u16,
    parser: &stypes::ParserType,
    filename: &Path,
) -> OperationResult<()> {
    let (tx_tail, rx_tail) = channel(1);
    let cancel = operation_api.cancellation_token();
    let (_, listening) = join!(
        tail::track(filename, tx_tail, cancel),
        super::run_source(
            operation_api,
            state,
            source,
            source_id,
            parser,
            None,
            Some(rx_tail)
        )
    );
    listening
}

/// Keeps the session in sync with the file it is linked to. The content available on linking
/// has already been indexed, so only appended content is processed here.
async fn tail_linked_file(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    source_id: u16,
    filename: &Path,
) -> OperationResult<()> {
    operation_api.processing();
    // Confirm: main file content has been read
    state.file_read().await?;
    let (tx_tail, mut rx_tail) = channel::<Result<(), tail::Error>>(1);
    let cancel = operation_api.cancellation_token();
    let (result, tracker) = join!(
        async {
            select! {
                res = async {
                    while let Some(update) = rx_tail.recv().await {
                        update.map_err(|err| stypes::NativeError {
                            severity: stypes::Severity::ERROR,
                            kind: stypes::NativeErrorKind::Interrupted,
                            message: Some(err.to_string()),
                        })?;
                        state.update_session(source_id).await?;
                    }
                    Ok(())
                } => res,
                _ = cancel.cancelled() => Ok(())
            }
        },
        tail::track(filename, tx_tail, cancel.clone()),
    );
    result
        .and_then(|_| {
            tracker.map_err(|e| stypes::NativeError {
                severity: stypes::Severity::ERROR,
                kind: stypes::NativeErrorKind::Interrupted,
                message: Some(format!("Tailing error: {e}")),
            })
        })
        .map(|_| None)
}

fn input_file(filename: &Path) -> Result<File, stypes::NativeError> {
    File::open(filename).map_err(|e| stypes::NativeError {
        severity: stypes::Severity::ERROR,
        kind: stypes::NativeErrorKind::Io,
        message: Some(format!(
            "Fail open file {}: {}",
            filename.to_string_lossy(),
            e
        )),
    })
}
