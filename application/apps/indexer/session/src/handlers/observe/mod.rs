mod export_raw;
mod session;

use crate::{
    operations::{OperationAPI, OperationResult},
    state::SessionStateAPI,
    tail,
};
use components::Components;
use processor::producer::{MessageProducer, sde::*};
use std::{path::Path, sync::Arc};
use stypes::{SessionAction, SessionSetup};
use tokio::{
    join,
    sync::mpsc::{Receiver, Sender, channel},
};

pub async fn observing(
    operation_api: OperationAPI,
    state: SessionStateAPI,
    options: SessionSetup,
    components: Arc<Components>,
    rx_sde: Option<SdeReceiver>,
) -> OperationResult<()> {
    match &options.origin {
        SessionAction::File(file) => {
            observe_file(file, operation_api, state, &options, &components).await
        }
        SessionAction::Source => {
            let (desciptor, source, parser) = components.setup(&options)?;
            let mut logs_buffer =
                session::LogsBuffer::new(state.clone(), state.add_source(desciptor).await?);
            let producer = MessageProducer::new(parser, source);
            Ok(session::run_producer(
                operation_api,
                state,
                producer,
                &mut logs_buffer,
                None,
                rx_sde,
            )
            .await?)
        }
        SessionAction::Files(files) => {
            // Replacement of concat feature
            for file in files {
                let session_option = options.inherit(SessionAction::File(file.to_owned()));
                observe_file(
                    file,
                    operation_api.clone(),
                    state.clone(),
                    &session_option,
                    &components,
                )
                .await?;
            }
            Ok(Some(()))
        }
        SessionAction::ExportRaw(files, ranges, output) => {
            // We are creating one single buffer for all files to keep tracking ranges and current index
            let mut logs_buffer = export_raw::ExportLogsBuffer::new(output, ranges.clone())?;
            for file in files {
                if operation_api.cancellation_token().is_cancelled() {
                    return Ok(Some(()));
                }
                let (_, source, parser) =
                    components.setup(&options.inherit(SessionAction::File(file.to_owned())))?;
                let producer = MessageProducer::new(parser, source);
                export_raw::run_producer(operation_api.clone(), producer, &mut logs_buffer).await?;
            }
            Ok(Some(()))
        }
    }
}

async fn observe_file(
    file_path: &Path,
    operation_api: OperationAPI,
    state: SessionStateAPI,
    options: &SessionSetup,
    components: &Arc<Components>,
) -> OperationResult<()> {
    let (desciptor, source, parser) = components.setup(options)?;
    let mut logs_buffer =
        session::LogsBuffer::new(state.clone(), state.add_source(desciptor).await?);
    let producer = MessageProducer::new(parser, source);

    let (tx_tail, rx_tail): (
        Sender<Result<(), tail::Error>>,
        Receiver<Result<(), tail::Error>>,
    ) = channel(1);

    let (_, res) = join!(
        tail::track(file_path, tx_tail, operation_api.cancellation_token()),
        session::run_producer(
            operation_api,
            state,
            producer,
            &mut logs_buffer,
            Some(rx_tail),
            None // Files doesn't support SDE
        ),
    );
    res
}
