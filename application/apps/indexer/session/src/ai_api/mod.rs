
use super::*;

async fn ai_run(
    client_rx: tokio::sync::mpsc::Receiver<AiClientRequest>,
    server_rx: tokio::sync::mpsc::Receiver<AiServerRequest>,
    state_api: SessionStateAPI,
    tracker_api: OperationTrackerAPI,
    tx_callback_events: UnboundedSender<stypes::CallbackEvent>,
) {
    select! {
        Some(income_from_state) = client_rx.recv() => {},
        Some(income_form_ai_server) = server_rx.recv() => {
            match income_form_ai_server {
                TaskDef::Filter {..} => {
            state_api.set_matches()

                }
            }

        }
    }
}
