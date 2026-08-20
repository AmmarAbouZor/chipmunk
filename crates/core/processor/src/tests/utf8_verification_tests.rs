//! Verification of file content while the initial index is built.

use crate::{
    grabber::{ComputationResult, GrabError, GrabMetadata},
    text_source::TextFileSource,
};
use std::path::Path;
use tempfile::TempPath;

fn file_with(content: &[u8]) -> TempPath {
    use std::io::Write;

    let mut file = tempfile::NamedTempFile::new().expect("Could not create tmp file");
    file.write_all(content).expect("Could not write tmp file");
    file.flush().expect("Could not flush tmp file");
    file.into_temp_path()
}

fn index_verified(path: &Path) -> Result<GrabMetadata, GrabError> {
    let (result, _) = TextFileSource::verifying_utf8(path).from_file(None, None)?;
    match result {
        ComputationResult::Item(metadata) => Ok(metadata),
        ComputationResult::Stopped => panic!("Indexing was not cancelled"),
    }
}

fn invalid_offset(result: Result<GrabMetadata, GrabError>) -> u64 {
    match result {
        Err(GrabError::InvalidEncoding { offset }) => offset,
        other => panic!("Expected invalid encoding, got: {other:?}"),
    }
}

#[test]
fn verified_index_matches_unverified_one() {
    let path = file_with("first line\nsecond ünïcödé line\nthird line\n".as_bytes());

    let (unverified, _) = TextFileSource::new(&path)
        .from_file(None, None)
        .expect("Valid content is indexed");
    let unverified = unverified.into_option().expect("Indexing is not cancelled");

    assert_eq!(index_verified(&path).unwrap(), unverified);
}

#[test]
fn invalid_byte_is_reported_with_its_offset() {
    let mut content = b"first line\n".to_vec();
    let offset = content.len() as u64;
    content.push(0xff);
    content.extend_from_slice(b"second line\n");
    let path = file_with(&content);

    assert_eq!(invalid_offset(index_verified(&path)), offset);
}

#[test]
fn invalid_byte_beyond_the_first_ten_kilobytes_is_reported() {
    const LINE: &str = "0123456789ABCDEF\n";

    let mut content = LINE.repeat(10_240 / LINE.len() + 1).into_bytes();
    assert!(content.len() > 10_240);
    let offset = content.len() as u64;
    content.push(0xff);
    content.extend_from_slice(b"tail line\n");
    let path = file_with(&content);

    assert_eq!(invalid_offset(index_verified(&path)), offset);
}

#[test]
fn character_truncated_by_the_end_of_file_is_accepted() {
    // The last character is cut off, as it happens while a file is being written.
    let mut content = b"first line\n".to_vec();
    content.extend_from_slice(&[0xe2, 0x82]);
    let path = file_with(&content);

    index_verified(&path).expect("Truncated trailing character is valid");
}

#[test]
fn character_split_across_read_chunks_is_accepted() {
    // A single line far longer than the read buffer, filled with three byte characters, so that
    // a character is guaranteed to cross the chunk boundary.
    let content = format!("{}\n", "€".repeat(100_000));
    let path = file_with(content.as_bytes());

    let metadata = index_verified(&path).expect("Split characters are valid");
    assert_eq!(metadata.line_count, 1);
}

#[test]
fn content_appended_while_tailing_is_not_verified() {
    let mut content = b"first line\n".to_vec();
    let path = file_with(&content);
    let base = index_verified(&path).expect("Valid content is indexed");

    content.push(0xff);
    content.extend_from_slice(b"second line\n");
    std::fs::write(&path, &content).expect("Could not extend tmp file");

    TextFileSource::verifying_utf8(&path)
        .from_file(Some(base), None)
        .expect("Invalid content is tolerated while tailing");
}
