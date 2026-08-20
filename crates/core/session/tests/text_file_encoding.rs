//! Observing text files depending on whether their content can be read as UTF-8.
//!
//! Valid content is served from the original file without copying it, invalid content is
//! transcoded into a session file of its own.

use std::{fs::OpenOptions, io::Write, path::Path, time::Duration};

use processor::grabber::LineRange;
use session::{session::Session, state::SessionFileOrigin};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

/// Time given to the file tracker, which polls the file once per second, to pick up appended
/// content.
const TAIL_TIMEOUT: Duration = Duration::from_secs(15);

struct ObservedFile {
    session: Session,
    events: UnboundedReceiver<stypes::CallbackEvent>,
    _dir: TempDir,
}

impl ObservedFile {
    /// Observes a file with the given content as text and waits until it is completely read.
    async fn start(content: &[u8]) -> (Self, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("observed.log");
        std::fs::write(&path, content).unwrap();

        let uuid = Uuid::new_v4();
        let (session, mut events) = Session::new(uuid).await.unwrap();
        session
            .observe(
                uuid,
                stypes::ObserveOptions::file(
                    path.clone(),
                    stypes::FileFormat::Text,
                    stypes::ParserType::Text(()),
                ),
            )
            .unwrap();
        wait_for(&mut events, |event| {
            matches!(event, stypes::CallbackEvent::FileRead)
        })
        .await;

        (
            Self {
                session,
                events,
                _dir: dir,
            },
            path,
        )
    }

    async fn origin(&self) -> SessionFileOrigin {
        self.session
            .get_state()
            .get_session_file_origin()
            .await
            .unwrap()
            .expect("Session file is set after the file is read")
    }

    async fn lines(&self) -> Vec<String> {
        let (rows, _) = self.session.get_state().get_stream_len().await.unwrap();
        if rows == 0 {
            return Vec::new();
        }
        self.session
            .grab(LineRange::from(0..=rows - 1))
            .await
            .unwrap()
            .0
            .into_iter()
            .map(|element| element.content)
            .collect()
    }

    /// Waits until the session reports at least `rows` lines.
    async fn wait_for_rows(&mut self, rows: u64) {
        let waiting = wait_for(
            &mut self.events,
            |event| matches!(event, stypes::CallbackEvent::StreamUpdated(len) if *len >= rows),
        );
        tokio::time::timeout(TAIL_TIMEOUT, waiting)
            .await
            .expect("Appended content is picked up by tailing");
    }
}

async fn wait_for(
    events: &mut UnboundedReceiver<stypes::CallbackEvent>,
    expected: impl Fn(&stypes::CallbackEvent) -> bool,
) {
    while let Some(event) = events.recv().await {
        if let stypes::CallbackEvent::OperationError { error, .. } = &event {
            panic!("Received operation error: {error:#?}");
        }
        if expected(&event) {
            return;
        }
    }
    panic!("Session events are exhausted before the expected event arrived");
}

fn append(path: &Path, content: &str) {
    OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
}

/// Content with a byte which is not valid UTF-8 in the middle of the second line.
fn content_with_invalid_byte(padding_lines: usize) -> Vec<u8> {
    let mut content = "first line\n".repeat(padding_lines).into_bytes();
    content.extend_from_slice("second ".as_bytes());
    content.push(0xf1);
    content.extend_from_slice(" line\nthird line\n".as_bytes());
    content
}

#[tokio::test(flavor = "multi_thread")]
async fn valid_file_is_linked_and_tailed() {
    let (mut observed, path) = ObservedFile::start(b"first line\nsecond line\n").await;

    assert!(matches!(
        observed.origin().await,
        SessionFileOrigin::Linked(linked) if linked == path
    ));
    assert_eq!(observed.lines().await, ["first line", "second line"]);

    append(&path, "third line\n");
    observed.wait_for_rows(3).await;

    assert_eq!(
        observed.lines().await,
        ["first line", "second line", "third line"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_file_is_transcoded_and_tailed() {
    let (mut observed, path) = ObservedFile::start(&content_with_invalid_byte(1)).await;

    assert!(matches!(
        observed.origin().await,
        SessionFileOrigin::Generated(_)
    ));
    assert_eq!(
        observed.lines().await,
        ["first line", "second \u{fffd} line", "third line"]
    );

    append(&path, "fourth line\n");
    observed.wait_for_rows(4).await;

    assert_eq!(
        observed.lines().await,
        [
            "first line",
            "second \u{fffd} line",
            "third line",
            "fourth line"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_byte_beyond_the_first_ten_kilobytes_is_transcoded() {
    // Files were classified by their first 10240 bytes only, so content turning invalid after
    // them used to be linked and served as raw bytes.
    let padding_lines = 10_240 / "first line\n".len() + 1;
    let content = content_with_invalid_byte(padding_lines);
    assert!(content.len() > 10_240);

    let (observed, _path) = ObservedFile::start(&content).await;

    assert!(matches!(
        observed.origin().await,
        SessionFileOrigin::Generated(_)
    ));
    let lines = observed.lines().await;
    assert_eq!(lines.len(), padding_lines + 2);
    assert_eq!(lines[padding_lines], "second \u{fffd} line");
    assert_eq!(lines[padding_lines + 1], "third line");
}
