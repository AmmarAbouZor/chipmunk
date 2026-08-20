# Implementation Summary: Transcode Non-UTF-8 Text Files Instead of Linking Them

Companion to `LINK_PLAN.md`. Describes what was implemented, how it deviates from the plan, the
measured cost, and where the result still feels weak.

Status: implemented in the working tree of `misc/`. Workspace compiles, `cargo fmt`, `cargo clippy
--workspace --all-targets` and `cargo test --workspace --exclude plugins_api` are clean.
(`plugins_api` fails on its pre-existing "tests were run without activating any features" guard,
unrelated to this work.) No git operation was performed.

## Result

A text file is linked as session file only when its content is valid UTF-8. Otherwise it goes
through the normal producer path (`BinaryByteSource` + `StringTokenizer` + tail) and is transcoded
into a generated session file, where invalid bytes become `U+FFFD`. The verdict is produced by the
indexing pass that already reads every byte, so the fast path costs no extra I/O.

## What changed

### Step 1: fused verification in the indexing pass

- `crates/core/processor/src/grabber/mod.rs`
  - New `GrabError::InvalidEncoding { offset: u64 }` and its `stypes::NativeError` conversion. The
    variant is consumed as a typed value by the linking code; nothing parses its message.
- `crates/core/processor/src/text_source.rs`
  - `TextFileSource` gained `verify_utf8`. `TextFileSource::new` stays the unverified constructor
    (generated session files, `grabber/factory.rs`, existing tests), `TextFileSource::verifying_utf8`
    is the new one.
  - Verification happens inside the existing `from_file` read loop, over exactly the slice that is
    about to be consumed, and only when `base.is_none()` (first pass). Content read while tailing is
    never verified, so a partially written character at the end of a growing file cannot invalidate a
    published session.
  - `Utf8Validator` carries an incomplete trailing character (at most 3 bytes) into the next chunk
    instead of reporting it, and completes it from the head of that chunk. A carry left over when the
    file ends counts as valid.
- `crates/core/processor/src/tests/utf8_verification_tests.rs` (new)
  - Verified index equals unverified index; exact offset of an invalid byte; invalid byte beyond the
    first 10240 bytes; character truncated at EOF accepted; character split across the 32 KiB read
    buffer inside a line longer than the buffer accepted; appended invalid content tolerated on an
    incremental pass.

### Step 2: link vs. create split in the session file

- `crates/core/session/src/state/session_file.rs`
  - `init()` creates a generated session file and its writer.
  - `link(path, source_id) -> LinkOutcome` builds a grabber over the verifying source, runs the
    initial index pass, registers the indexed line range for the source, and assigns `filename` and
    `grabber` only on success. On `GrabError::InvalidEncoding` it logs the offset and returns
    `LinkOutcome::NotUtf8` without touching session state, so no rollback API leaks out of the type.
  - `LinkOutcome { Linked, NotUtf8 }` is re-exported through `crate::state`.
- `crates/core/session/src/state/api.rs`, `crates/core/session/src/state/mod.rs`
  - `Api::SetSessionFile` replaced by `Api::CreateSessionFile` and `Api::LinkSessionFile`, with
    `SessionStateAPI::create_session_file` and `SessionStateAPI::link_session_file`.
  - `handle_link_session_file` sets the attachments destination and publishes the freshly indexed
    content through `update_searchers`.
- Callers: `handlers/observing/mod.rs` (`create_session_file`), `state/tests_nested.rs`,
  `tests/nested_search.rs`.

### Step 3: fallback in `observe_file`

- `crates/core/session/src/handlers/observing/file.rs`
  - The four duplicated `join!(tail::track(...), run_source(...))` blocks collapsed into one
    `produce_from_file` helper, used by `Binary`, `PcapLegacy`, `PcapNG`, the non-text-parser text arm
    and the new fallback.
  - The linked flow moved into `tail_linked_file`.
  - Text files with the text parser: `link_session_file` first, then either the linked flow or the
    producer path, depending on `LinkOutcome`.
- `crates/core/session/tests/text_file_encoding.rs` (new)
  - Valid file: origin `Linked`, content grabbed as written, appended lines picked up by tailing.
  - Invalid file: origin `Generated`, invalid byte grabbed as `U+FFFD`, lines before and after intact,
    appended lines picked up by tailing.
  - Invalid byte beyond the first 10240 bytes: origin `Generated`, full content present.

### Step 4: removal of the app-level 10 KB encoding gate

- `crates/app/src/host/service/file.rs`: `FileFormatDetection` and
  `unsupported_text_encoding_message` deleted, `detect_file_format` returns `io::Result<FileFormat>`,
  `scan_dir` filter simplified, module tests updated.
- `crates/app/src/host/service/mod.rs`: `open_single_file`, `open_files_with_plugin` and
  `open_multi_files` use the format directly; the partitioning and warning loop are gone.
- `crates/app/src/session/service/mod.rs`: `AttachSource::Files` maps paths to formats without the
  skip-and-warn branch.
- `crates/file_tools/src/lib.rs`: `is_binary` had no callers left and was deleted; its tests were
  rewritten against `is_utf8_text`.

Accepted consequence: a binary file carrying a text extension (a gzip named `.log`) now opens as a
text session full of replacement characters instead of being skipped with a warning.

## Deviations from the plan

1. **`link_session_file` takes the source id, and linking publishes the content.**
   The plan had `link_session_file(path)` followed by the existing `update_session` call. That does
   not work: after linking, the grabber already holds the metadata, so `SessionFile::update` sees an
   unchanged line count, reports `NoChanges`, and `handle_update_session` never emits
   `StreamUpdated` or the search requests. The initial line range would also never be attributed to
   the source. Linking therefore registers the range and the state handler runs `update_searchers`
   right away.
2. **The linked branch no longer calls `update_session` before `processing()`/`file_read()`.**
   The index is built during linking; the call was pure overhead. The window between the initial pass
   and the start of tailing is the same as before.
3. **`SessionFile::link` errors when a session file already exists** instead of silently returning
   `Linked`. `observe.rs` rejects that combination beforehand, so this is an invariant violation; a
   silent success would make the caller tail a file whose content was never read.
4. **Session-level tests run on the multi-thread tokio flavor.** The searchers task uses
   `block_in_place`, which panics on a current-thread runtime, so `#[tokio::test]` alone kills the
   searcher task in the background. The existing snapshot tests already do this.
5. **`file_tools::is_binary` was deleted** rather than kept, since the gate was its last caller.

## Measurements

Release build, warm page cache, single machine, 512 MB generated log file (~7.6 M lines).

### Cost of the fused verification (`TextFileSource::from_file`, 3 runs)

| pass | run 1 | run 2 | run 3 |
| --- | --- | --- | --- |
| index only (`new`) | 77.6 ms | 75.1 ms | 74.8 ms |
| index + verify (`verifying_utf8`) | 191.0 ms | 190.9 ms | 191.3 ms |

Verification adds ~116 ms per 512 MB, that is ~230 ms per GB, and roughly 2.5x the CPU cost of the
pass. Effective verification throughput is ~4.4 GB/s, which is `std::str::from_utf8` running its
ASCII fast path. For reference, the plan's planning machine measured ~236 ms per 512 MB for the
index-only pass, so the verified pass here is still below that number, and cold-cache opens remain
I/O bound.

### End-to-end session open (observe until `FileRead`)

| file | time | session file |
| --- | --- | --- |
| 512 MB valid UTF-8 | 209 ms | `Linked` (no copy) |
| same file, one invalid byte at offset 200 | 1.19 s | `Generated` (full transcode) |

The fallback costs ~5.7x the open time and one file-size of temp disk, as expected: a partial index
pass up to the first invalid byte, then a full parse, transcode, write and re-index.

Both measurements were taken with throwaway `#[ignore]` tests that were removed again; the generated
input files were deleted.

## Assessment

What I think is solid:

- Verification is genuinely free of extra I/O and sits in the one place that already touches every
  byte. The carry logic keeps the "every byte is validated exactly once" invariant, including the
  long-line branch where a chunk can split a character.
- `LinkOutcome` keeps the decision typed. No caller inspects an error message, and a rejected file
  never leaves partial state behind, because `link` assigns to `self` only after the pass succeeded.
- `observe_file` lost three copies of the same `join!` block, so the new branch did not add a fourth.
- The behavior change is covered end to end (origin, content, tailing) rather than only at the
  processor level.

What I am less happy about, in rough order of importance:

- **Publishing on link.** `handle_link_session_file` now calls `update_searchers` directly, which
  makes two places in the state responsible for announcing new content (`handle_update_session` is
  the other). It is correct, but the "content was produced" notification really wants to be one
  concept instead of two call sites.
- **`source_id` in `SessionFile::link`.** It is there only to register the initial range, mirroring
  `SessionFile::update`. It is honest, but it makes the linking API carry a session concern.
- **Verification cost.** 2.5x on the CPU part of the index pass is real, even if disk I/O hides it on
  cold opens. `simdutf8` would remove it almost entirely, but it is a new workspace dependency, so I
  left the decision open rather than adding it silently.
- **The gate removal is a genuine regression for one case:** binary content behind a text extension
  now opens as a wall of `U+FFFD` instead of a warning. This was decision 2 of the plan and I kept
  it, but it is the change most likely to generate user reports.
- **`StreamUpdated(0)` for an empty linked file** is now emitted where nothing was emitted before.
  Harmless as far as I can tell, but it is a small behavioral difference nobody asked for.
- **`TextFileSource::valid_lines` still decodes lossily** with `String::from_utf8_lossy`. It is now
  dead weight for correctness (both session file kinds are valid UTF-8) but I left it alone, since
  removing it belongs to the encoding-support work, not here.

## Follow-ups worth considering

- Decide on `simdutf8` with a cold-cache measurement on a real multi-GB file.
- Real encoding support in `StringTokenizer` (`crates/core/parsers/src/text.rs`), which is now the
  single decode seam; `encoding_rs` for Windows-1252 and UTF-16 would turn today's `U+FFFD` output
  into readable text.
- `LinesCodec` UTF-8 failures in `crates/core/sources/src/command/process.rs` remain untouched.
- A changelog entry: `changelog.md` has no unreleased section, so nothing was added.
