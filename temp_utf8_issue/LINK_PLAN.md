# Implementation Plan: Transcode Non-UTF-8 Text Files Instead of Linking Them

## Status

Planned, not implemented. Everything below was verified against the code in `misc/` at planning time.
Paths in this document are relative to the repository root that contains `crates/`.

Do not run any git operation while implementing this plan: no commit, no staging, no branching, no
stash, no reset. Leave all changes in the working tree.

## Problem

When a text file is opened with the text parser, Chipmunk does not copy it. It links the original file
as the session file (`SessionFileOrigin::Linked`) and only builds a line index over it. Every consumer
(grabbing, searching, extraction, export) then reads the original bytes.

Those bytes are not guaranteed to be valid UTF-8, while the consumers assume valid UTF-8. The decoding
policy is inconsistent across consumers: display converts lossily (`String::from_utf8_lossy` in
`TextFileSource::valid_lines`), so a user can see a line that other components cannot process.

Generated session files do not have this problem. They are written from Rust `String` values by
`LogsWriter` (`writeln!`), so their content is valid UTF-8 by construction. That path is already used
for streams, concat, DLT, plugins, and for text files added to an already-running session.

## Goal

Guarantee that the session file is always valid UTF-8, without giving up the zero-copy fast path for
the common case.

Decision rule:

- file is valid UTF-8 -> link it, exactly as today (no copy, no extra read);
- file is not valid UTF-8 -> do not link; run it through the normal producer path
  (`BinaryByteSource` + `StringTokenizer` + tail), which lossily transcodes into a generated
  session file.

This also creates a single decode boundary (`StringTokenizer`), which is the place where real
encoding support (`encoding_rs` for Windows-1252, UTF-16, ...) can be added later. Adding such support
is **not** part of this work; invalid bytes become `U+FFFD`.

## Core idea: validation is free

The linked path already reads every byte of the file to build the line index
(`TextFileSource::from_file`, called through `Grabber`). Fuse UTF-8 validation into that existing pass
and fail fast at the first invalid byte.

Rejected alternative: a separate validation scan before linking. It costs a second full read of the
file, which is a 2x open-time regression for the multi-GB files that linking exists for.

At the moment the verdict is known, nothing has been published to the UI yet: `handle_update_session`
(`crates/core/session/src/state/mod.rs`) emits `CallbackEvent::StreamUpdated` and search requests only
after `SessionFile::update` reports `SessionFileState::Changed`. So abandoning a link means dropping
the grabber and the origin; no session state has to be unwound.

## Agreed decisions

1. **Validate the first pass only.** Once content is published, line numbers and byte offsets are live,
   so switching to transcoding mid-session is impossible. A file that becomes invalid while tailing
   (rotation, partial write of a multibyte character) must be tolerated, not rejected.
2. **Remove the app-level 10 KB encoding gate** (`FileFormatDetection::UnsupportedTextEncoding`).
   Without this, the new fallback is only reachable for files that turn invalid after the first
   10,240 bytes, and most of the benefit is unrealized.
3. **Files without a text extension are out of scope.** A file with invalid initial bytes and a
   non-text extension (`.bin`, no extension, ...) is classified `FileFormat::Binary` today and keeps
   that behavior.
4. **Do not touch the search code** (`crates/core/processor/src/search/**`, `crates/core/text_grep/**`).
   This change is only about how session content is produced.

## Current code map

Read these before starting:

- `crates/core/processor/src/text_source.rs`
  - `TextFileSource { path: PathBuf }`, constructed by `TextFileSource::new(&Path)`.
  - `from_file(base: Option<GrabMetadata>, shutdown_token) -> Result<(ComputationResult<GrabMetadata>, Option<RangeInclusive<u64>>), GrabError>`
    is the indexing pass. It reads through `bufread::BufReader` with `BIN_READER_CAPACITY = 32 KiB`
    and `BIN_MIN_BUFFER_SPACE = 10 KiB`, counts `b'\n'`, and builds `Slot`s.
  - `valid_lines()` converts read bytes with `String::from_utf8_lossy` for display.
- `crates/core/processor/src/grabber/mod.rs`
  - `GrabError`, `ComputationResult`, `GrabMetadata`, `Slot`.
  - `Grabber::lazy(source)` (no read), `Grabber::create_metadata(token)` (first pass),
    `Grabber::update_from_file(token)` (incremental pass), `Grabber::inject_metadata(md)`.
- `crates/core/session/src/state/session_file.rs`
  - `SessionFileOrigin { Linked(PathBuf), Generated(PathBuf) }`, `is_linked()`.
  - `SessionFile::init(filename: Option<PathBuf>)`: `Some` -> `Linked` + `Grabber::lazy`;
    `None` -> creates `<uuid>.session` in the streams dir, opens the writer, `Generated`.
    Early-returns when `self.grabber.is_some()`.
  - `SessionFile::update(...)` -> `grabber.update_from_file(...)` -> `TextFileSource::from_file(...)`.
  - `cleanup()` deletes generated files only.
- `crates/core/session/src/state/api.rs`
  - `Api::SetSessionFile((Option<PathBuf>, oneshot))`, `SessionStateAPI::set_session_file`,
    `Api::UpdateSession`, `SessionStateAPI::update_session`, plus the debug-name mapping around
    line 209.
- `crates/core/session/src/state/mod.rs`
  - `Api::SetSessionFile` handler (~line 625) also calls `state.attachments.set_dest_path(filename)`.
  - `handle_update_session` (~line 361) and `update_searchers` (~line 379).
- `crates/core/session/src/handlers/observing/file.rs`
  - `observe_file`. The `FileFormat::Text` arm links the file when the parser is
    `ParserType::Text(())`; otherwise it uses the producer path. The
    `join!(tail::track(...), run_source(...))` block is duplicated four times in this file.
- `crates/core/session/src/handlers/observing/mod.rs`
  - `run_source` -> `run_producer`, and `state.set_session_file(None).await?` around line 145.
- `crates/app/src/host/service/file.rs`, `crates/app/src/host/service/mod.rs`,
  `crates/app/src/session/service/mod.rs`, `crates/file_tools/src/lib.rs`
  - The 10 KB encoding gate.

## Changes

### Step 1: fused UTF-8 validation in the indexing pass

**`crates/core/processor/src/grabber/mod.rs`**

- Add a variant carrying the position of the first invalid byte:

  ```rust
  #[error("Invalid UTF-8 content at byte {offset}")]
  InvalidEncoding { offset: u64 },
  ```

- Extend the `From<GrabError> for stypes::NativeError` match accordingly
  (`NativeErrorKind::ComputationFailed` is fine). This conversion is a fallback only; the linking code
  handles the variant as a typed value and never lets it reach the UI. Do not add any logic anywhere
  that inspects error message text.

**`crates/core/processor/src/text_source.rs`**

- Add an opt-in flag to the source, e.g.:

  ```rust
  pub struct TextFileSource {
      path: PathBuf,
      /// Content of an external file is verified while the initial index is built.
      /// Generated session files are valid by construction and skip verification.
      verify_utf8: bool,
  }
  ```

  Keep `TextFileSource::new(&Path)` as the non-verifying constructor (used by generated session files,
  `grabber/factory.rs` and the existing tests) and add a second constructor for the verifying variant.

- Validate inside the `from_file` read loop, and only when `base.is_none()` (first pass; agreed
  decision 1). Document that invariant on the constructor or the field.

- Validation rules per iteration, applied to the slice that is about to be consumed
  (`&content[..consumed]`):
  - `nl > 0` branch: the consumed slice ends with `b'\n'`, so an incomplete multibyte sequence cannot
    be a boundary artifact. Any `str::from_utf8` error is a real error.
  - `nl == 0` branch (a line longer than the 32 KiB read buffer): the slice can split a character. On
    `Err(e)` with `e.error_len().is_none()`, the tail from `e.valid_up_to()` is an incomplete sequence:
    carry those bytes and re-validate them prepended to the next chunk instead of failing.
  - Loop end with a non-empty carry (truncated sequence at EOF, i.e. a file currently being written)
    counts as valid.
  - Any other error: return `Err(GrabError::InvalidEncoding { offset })` immediately, where `offset` is
    the absolute file position (`byte_offset + e.valid_up_to()`), without finishing the pass.

- Coverage invariant to preserve: every byte is validated exactly once. Bytes after the last newline
  stay in the reader buffer and are presented again by the next `fill_buf`, so validating only the
  consumed slice of each iteration covers the whole file.

- Performance note: `std::str::from_utf8` is roughly 1 GB/s, while `bytecount` is SIMD and much faster,
  so validation can dominate the CPU cost of this pass. Measure (see "Measurements"). Only if a
  regression is measured, switch this call to the `simdutf8` crate (not currently a dependency).

**Tests (`crates/core/processor/src/tests/grabber_tests.rs` or a new module)**

- valid file: verifying source produces the same metadata as the non-verifying one;
- invalid byte in the middle: `InvalidEncoding` with the exact offset, and the pass stops early;
- invalid byte after the first 10,240 bytes (the reported real-world case);
- truncated multibyte sequence at EOF: valid;
- multibyte character split across the 32 KiB buffer boundary inside a line longer than the buffer:
  valid (exercises the carry path);
- incremental pass (`base = Some(_)`) over content with invalid bytes: no error (validation is
  first-pass only).

### Step 2: split linking from creating in the session file

**`crates/core/session/src/state/session_file.rs`**

- Replace `init(Option<PathBuf>)` with two explicit operations, removing the `Option`-as-mode-switch:

  ```rust
  /// Creates a new generated session file with its writer.
  pub fn init(&mut self) -> Result<(), stypes::NativeError>

  /// Links an existing file as session file. The initial index is built here, and the file is
  /// rejected when its content is not valid UTF-8.
  pub fn link(&mut self, path: PathBuf) -> Result<LinkOutcome, stypes::NativeError>
  ```

  ```rust
  pub enum LinkOutcome {
      Linked,
      /// Content is not valid UTF-8 and must be transcoded into a generated session file.
      NotUtf8,
  }
  ```

- `link` builds the grabber over the **verifying** `TextFileSource`, sets `filename` to
  `SessionFileOrigin::Linked(path)`, and runs the initial index pass immediately
  (`Grabber::create_metadata`). On `GrabError::InvalidEncoding` it clears `self.grabber` and
  `self.filename` (log the offset at debug/warn level) and returns `LinkOutcome::NotUtf8`, so no
  rollback API leaks outside this type. Other grab errors keep propagating as `NativeError`.
- Keep the existing "already initialized" guard behavior for both operations.
- After a successful `link`, the first `SessionFile::update` call is a cheap tail scan because the
  metadata is already present, so no work is repeated.

**`crates/core/session/src/state/api.rs` and `crates/core/session/src/state/mod.rs`**

- Replace `Api::SetSessionFile((Option<PathBuf>, oneshot))` with:
  - `Api::CreateSessionFile(oneshot)` -> `SessionStateAPI::create_session_file()`;
  - `Api::LinkSessionFile((PathBuf, oneshot))` -> `SessionStateAPI::link_session_file(path) -> Result<LinkOutcome, NativeError>`.
- Update the debug-name mapping (`api.rs` ~line 209).
- Both handlers keep calling `state.attachments.set_dest_path(filename)` on success. In the `NotUtf8`
  case nothing is set, and the subsequent `create_session_file()` from `run_source` sets it to the
  generated file.
- The initial index pass now happens while handling `LinkSessionFile` rather than `UpdateSession`.
  Both run on the same state actor task, so blocking behavior is unchanged.

**Callers to update**

- `crates/core/session/src/handlers/observing/mod.rs` (~line 145): `set_session_file(None)` ->
  `create_session_file()`.
- `crates/core/session/tests/nested_search.rs` (~line 33): `set_session_file(Some(path))` ->
  `link_session_file(path)`.

### Step 3: fall back to the producer path in `observe_file`

**`crates/core/session/src/handlers/observing/file.rs`**

- Extract the repeated `join!(tail::track(...), run_source(...))` block into one helper used by the
  `Binary`, `PcapLegacy`, `PcapNG`, non-text-parser text arms and the new fallback. Do not add a fifth
  copy of it.
- The text arm becomes:

  ```
  if parser is not Text  -> producer path (unchanged behavior)
  match state.link_session_file(filename).await? {
      LinkOutcome::Linked  -> existing linked flow: update_session, processing(), file_read(), tail loop
      LinkOutcome::NotUtf8 -> producer path (BinaryByteSource + text parser + tail)
  }
  ```

- Nothing else in the linked flow changes. The producer branch already handles tailing through
  `rx_tail`.

**Tests (`crates/core/session/tests/`, alongside `nested_search.rs` / `snapshot_tests`)**

- observing a valid text file: origin is `Linked`, grabbed content and line count unchanged;
- observing a text file with invalid bytes: origin is `Generated`, the invalid bytes are grabbed as
  `U+FFFD`, and the session content is complete (lines before and after the invalid one are present);
- tailing works in both cases (append to the file after the initial read and assert the new lines
  arrive).

Fixtures: `temp_utf8_issue/test_inputs/windows-none-utf8-formats.txt` has its first invalid byte at
offset `0xCC`, which suits the processor-level tests. Add a second fixture with more than 10 KiB of
valid ASCII before the first invalid byte for the "invalid after the old gate" case; generate it inside
the test rather than adding a large binary file to the repository.

### Step 4: remove the app-level 10 KB encoding gate

The gate exists only because the linked path could not handle invalid bytes. After step 3 it rejects
files that the session can now open.

**`crates/app/src/host/service/file.rs`**

- Delete `FileFormatDetection` and `unsupported_text_encoding_message`. `detect_file_format` returns
  `io::Result<FileFormat>` and classifies as: valid UTF-8 prefix -> `Text`; `.pcap` -> `PcapLegacy`;
  `.pcapng` -> `PcapNG`; text extension (`is_text_extension`) -> `Text`; otherwise -> `Binary`.
- Simplify the `scan_dir` filter accordingly (drop the skip-and-warn branch).
- Update the module tests: `detect_utf8_text_file`, `detect_binary_for_non_text_extension`,
  `detect_pcap_formats_before_text_encoding_diagnostic` lose the wrapper variant;
  `detect_unsupported_text_encoding_by_extension` becomes an assertion that a non-UTF-8 file with a
  text extension is detected as `FileFormat::Text`.

**`crates/app/src/host/service/mod.rs`**

- `open_single_file` (~line 431): use the returned format directly; drop the error branch.
- `open_files_with_plugin` (~line 505): drop the `UnsupportedTextEncoding -> Binary` mapping.
- `open_multi_files` (~line 549): drop the partitioning into `unsupported_text_files` and the warning
  loop (~line 575); keep the existing `files.is_empty()` early return.

**`crates/app/src/session/service/mod.rs`**

- `AttachSource::Files` (~line 550): drop `unsupported_text_files` and the warning notifications.

**`crates/file_tools/src/lib.rs`**

- `is_utf8_text` stays; it is still the primary text-vs-binary heuristic in `detect_file_format`.
  Check whether `is_binary` still has callers after this step and delete it if it does not.

Consequence to be aware of: a binary file carrying a text extension (for example a gzip named `.log`)
now opens as a text session full of replacement characters instead of being skipped with a warning.
This is the accepted tradeoff of decision 2.

## Measurements

1. Open time for a large (>= 1 GB) valid UTF-8 text file, before vs after step 1, to price the fused
   validation. Reference numbers on the planning machine (512 MB, warm cache, std-only proxy):
   index-only pass ~236 ms. If validation shows a clear regression, evaluate `simdutf8`.
2. Open time and temp disk usage for an invalid file, to confirm the fallback cost is acceptable: one
   partial index pass up to the first invalid byte, then a full transcode and copy (proxy measurement:
   ~520 ms transcode + ~225 ms index per 512 MB, plus one file-size of temp disk).

## Implementation order

Implement the steps in the order given above, keeping the workspace compiling and the tests passing
after each one:

1. Fused validation in `TextFileSource` + `GrabError::InvalidEncoding` + processor tests. Self-contained,
   no behavior change for existing callers because verification is opt-in.
2. `SessionFile::init`/`link` split, state API split (`CreateSessionFile` / `LinkSessionFile`), caller
   updates. No user-visible change yet.
3. `observe_file` fallback plus the `join!` deduplication, session-level tests. This is where behavior
   changes.
4. Removal of the app-level encoding gate and its tests.

## Out of scope

- Real encoding support (Windows-1252, UTF-16, Shift-JIS). Invalid bytes become `U+FFFD`.
  `StringTokenizer` (`crates/core/parsers/src/text.rs`, already marked `TODO: support non-utf8
  encodings`) is the single seam where this would be added later.
- Any change to search or grep behavior.
- Files with invalid content and a non-text extension.
- The `LinesCodec` UTF-8 failure in `crates/core/sources/src/command/process.rs` (process stdout/stderr
  ingestion). Separate issue.
