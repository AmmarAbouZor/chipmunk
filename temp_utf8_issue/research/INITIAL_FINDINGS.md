# Initial Findings: Non-UTF-8 Search and Stream Reading

## Purpose and scope

This document records the encoding research completed before implementation. It is intended to let another agent continue without repeating the initial repository investigation.

Work is intentionally split into two issues, in this order:

1. **Search resilience is the current priority.** Benchmark the existing UTF-8 search path, then evaluate replacing strict grep sinks with lossy sinks. `RegexSet` is a later, optional optimization.
2. **Stream-reading failures are deferred.** The process-output UTF-8 failure is a separate ingestion issue and must not be mixed into the search change.

The immediate search goal is best-effort operation in the presence of invalid UTF-8 bytes. It is **not** correct support for every source encoding. In particular, this work does not aim to make arbitrary Windows code pages, BOM-less UTF-16, or other unknown encodings searchable as correctly decoded Unicode.

No user warning for lossy conversion is currently required.

A test input has been added for later work:

- `temp_utf8_issue/test_inputs/windows-none-utf8-formats.txt`

It has not yet been used to validate or benchmark a change.

## Reported search failure

The observed error is a `SearchError::IoOperation` containing text similar to:

```text
Could not search in file ...; error: invalid utf-8 sequence of 1 bytes from index ...
```

The failure originates from `grep_searcher::sinks::UTF8`, not from an up-front validation of the complete file.

The workspace currently locks:

- `grep-searcher 0.1.16`
- `grep-regex 0.1.14`

In `grep-searcher 0.1.16`, the `UTF8` sink calls `std::str::from_utf8(mat.bytes())`. It returns an `io::Error` if the emitted match bytes are not valid UTF-8. The adjacent `Lossy` sink instead uses `String::from_utf8_lossy` and substitutes invalid sequences with `U+FFFD`.

## Search failure locations

### Match extraction

File:

- `crates/core/processor/src/search/extractor.rs`

`MatchesExtractor::extract_matches()`:

1. Converts all filters to regex text.
2. Combines them into one alternation.
3. Uses `Searcher::new().search_path(...)` with `sinks::UTF8`.
4. Passes each emitted line to `get_extracted_value()`.
5. Converts sink/search errors into `SearchError::IoOperation`.

This path is used by chart extraction through:

- `crates/core/session/src/handlers/extract.rs`
- `OperationKind::Extract` in `crates/core/session/src/operations.rs`

An error discards the extraction operation's accumulated output.

### Regular and value search

File:

- `crates/core/processor/src/search/searchers/mod.rs`

`BaseSearcher::search()` is shared by:

- Regular filtering in `search/searchers/regular.rs`
- Numeric value search in `search/searchers/values.rs`

It:

1. Opens the session file.
2. Seeks to the searcher's previous source-byte position.
3. Wraps the range in `CancellableBufReader` and `Read::take`.
4. Builds one combined `RegexMatcher`.
5. Uses `Searcher::new().search_reader(...)` with `sinks::UTF8`.
6. Invokes a regular or value collector for each emitted matching line.
7. Converts sink/search errors into `SearchError::IoOperation`.

`bytes_read` and `lines_read` are updated only after `search_reader` succeeds. A UTF-8 failure therefore does not advance the holder. Regular and value operation callers return an error rather than partial successful output.

For later observing updates, search errors received by the session state are currently logged rather than returned as successful partial updates.

### `text_grep`

File:

- `crates/core/text_grep/src/lib.rs`

`process_file()` also uses `sinks::UTF8`. Its corresponding error is `GrepError::FileProcessingError`, not `SearchError::IoOperation`.

There are currently no production callers of `count_occurrences`; its callers are tests. The processor depends on `text_grep` for `CancellableBufReader`. This sink should be considered for consistency, but it is lower priority than the processor search APIs.

## Why invalid bytes fail only on matching lines

The current grep pipeline is approximately:

```text
source bytes
  -> optional BOM/explicit-encoding decoder
  -> grep matcher scans for a candidate line
  -> no match: continue without invoking the sink
  -> match: emit the complete matching line to the sink
  -> UTF8 sink validates the emitted line
  -> Chipmunk collector receives &str
```

The sink is a consumer of search events, not a validator or formatter for every line.

Consequences:

- Invalid bytes in a non-matching line are not checked by `sinks::UTF8`.
- An ASCII term can match a line that also contains an invalid byte. The complete line then reaches the sink and fails UTF-8 validation.
- If invalid encoding prevents the primary matcher from recognizing a term, the sink is never called and the result can be silently missed.
- The index in the UTF-8 error refers to the emitted match bytes, generally the matching line, rather than proving that the whole file was validated.

Example:

```text
normal line
unrelated <0xFF> content
ERROR <0xFF> occurred
```

Searching for `ERROR` ignores the invalid second line but fails when the third line reaches `sinks::UTF8`.

## Why Chipmunk runs regex matching twice

The processor uses a candidate/classification design.

### First phase: candidate selection

All filter regexes are manually combined with `|`. `grep-searcher` scans the source once and emits a line when at least one filter matches.

### Second phase: classification and captures

Chipmunk then runs individual `regex::Regex` instances over each candidate line.

Regular search uses the second phase to determine:

- Every matching filter ID for the line
- Per-filter statistics
- Aliases associated with matching filters

Value search and match extraction use it to obtain capture groups and numeric values. A line may match multiple filters, including overlapping filters, so a simple leftmost alternation does not preserve the current semantics.

The approximate work is:

```text
all input x one combined matcher
candidate lines x individual filter regexes
```

This is reasonable when filter counts are small and matches are sparse. It can become expensive with many filters or broad filters matching most lines.

`grep-searcher::SinkMatch` exposes line bytes, line number, and offsets, but not the matching pattern ID or captures. `RegexMatcherBuilder::build_many()` is available and better expresses multiple-pattern construction than manual string joining, but it still behaves as an alternation and does not give the sink all matching filter IDs.

## Current session-file and display behavior

For a normal file using `FileFormat::Text` and the text parser, `observe_file()` links the original source file directly as the session file:

- `crates/core/session/src/handlers/observing/file.rs`

It does not pass the file through `StringTokenizer`.

`TextFileSource` in `crates/core/processor/src/text_source.rs`:

- Counts and indexes lines using raw bytes and raw `0x0A` newline detection.
- Reads selected byte segments for display.
- Converts displayed content using `String::from_utf8_lossy`.

This creates an existing inconsistency:

- A line can be indexed and displayed with replacement characters.
- Search operates on the original bytes and can fail strictly when the same line matches.

Generated session files differ. Parsed messages are accumulated in Rust `String` values before being written, so generated session files contain valid UTF-8. `StringTokenizer` also uses `String::from_utf8_lossy`, but it is bypassed for the directly linked text-file path.

## Existing encoding checks and notes

Relevant repository landmarks:

- `crates/core/parsers/src/text.rs`
  - Contains `TODO: support non-utf8 encodings`.
  - Uses `String::from_utf8_lossy`.
- `crates/core/processor/src/text_source.rs`
  - States that non-UTF-8 coding is unsupported and converts content lossily before returning strings.
- `crates/file_tools/src/lib.rs`
  - Classifies only the first 10,240 bytes with `str::from_utf8`.
- `crates/app/src/host/service/file.rs`
  - Reports known text extensions with an invalid initial UTF-8 chunk as `UnsupportedTextEncoding`.
- `crates/core/README.md`
  - Historical changelog entries mention support for files containing invalid UTF-8 and avoiding UTF-8 validity checks.

The application-level initial-chunk check does not guarantee that the complete file is valid. Invalid bytes after the initial chunk can reach the linked-text search path. The fixed chunk can also end inside a valid multibyte sequence because its boundary is not UTF-8-aware.

An adjacent but separate encoding-sensitive location is `crates/core/merging/src/merger.rs`, which uses `std::str::from_utf8_unchecked` on line bytes. It is not part of the reported search failure and is not part of the current planned work.

## Expected behavior of `Lossy`

Replacing `sinks::UTF8` with `sinks::Lossy` prevents the sink from stopping at an invalid emitted line. Invalid sequences in that line become `U+FFFD`.

Expected benefits:

- Search can continue after invalid matching lines.
- Valid results before and after such lines can be retained.
- ASCII searches over mostly ASCII-compatible data often continue to work.
- Regular, value, and extraction collectors continue receiving `&str`.

Important limits:

- Lossy conversion happens after the primary grep matcher selects a candidate line.
- It does not decode Windows-1252, UTF-16, Shift-JIS, or another unknown encoding.
- A non-ASCII search term encoded differently in the source will generally not be recognized by the primary matcher.
- The first matcher may inspect raw bytes while the second-phase regexes inspect the lossy string, so the two phases can disagree.
- Lossy is a resilience measure, not general encoding support.

No lossy-conversion warning needs to be added to operation results or UI in the current scope.

## Encoding-specific behavior

### Valid UTF-8

`Lossy` first attempts the same UTF-8 validation as `UTF8`. Valid text is returned as borrowed data with no allocation. Search behavior should remain the same.

### Malformed UTF-8

If a candidate line contains invalid UTF-8, `Lossy` allocates a converted string and replaces invalid sequences. ASCII filters that do not depend on the corrupted region will often still classify the line correctly.

### Windows-1252 and similar ASCII-compatible encodings

ASCII portions use compatible bytes, so ASCII filters can often select candidate lines. Non-ASCII bytes are not decoded as Windows-1252; they may become replacement characters. UTF-8 search terms containing those non-ASCII characters generally do not match the source bytes and therefore never reach the sink.

### UTF-16 with a BOM

`Searcher::new()` enables BOM sniffing by default. When searching a range that starts at the UTF-16 BOM, `grep-searcher` can transcode the source to UTF-8 before matching. In that case, `Lossy` receives valid UTF-8 and is mostly irrelevant.

### UTF-16 without a BOM or incremental ranges

Without a BOM or explicit encoding, UTF-16 bytes are treated as if they were UTF-8. ASCII code units contain interleaved NUL bytes, so ordinary multi-character queries do not match.

Incremental `BaseSearcher` calls create a new `Searcher` and start after the original BOM. They therefore cannot rely on initial BOM detection. Raw line indexing can also split UTF-16 code units. Correct UTF-16 support would require encoding-aware decoding, line boundaries, and persistent incremental decoder state, not a sink replacement.

## Performance expectations for `Lossy`

### Valid UTF-8

For each emitted matching line, both sinks perform UTF-8 validation. On valid text, `Lossy` returns borrowed data and does not allocate. Its extra `Cow` branch is expected to be negligible relative to matching and second-phase regex work.

### Invalid matching lines

For each invalid emitted line, `Lossy`:

1. Encounters the failed UTF-8 validation.
2. Scans and converts the line lossily.
3. Allocates a new UTF-8 string.
4. Writes replacement characters for invalid sequences.
5. Drops the temporary allocation after the callback unless derived values are retained.

The overhead therefore depends on invalid matching-line count and line length. Broad filters matching nearly every line are the worst case. Replacement characters can also make the converted string larger than the source bytes.

A total-duration comparison must account for the fact that the current strict sink stops early on invalid input while the lossy implementation completes the search.

## Existing test and benchmark coverage

There are no dedicated search or grep benchmarks.

Existing Criterion benchmarks cover:

- Processor producers
- Map scaling
- Plugin initialization and parsing
- Buffer and reader behavior

`crates/core/processor/benches/text_producer.rs` benchmarks `MessageProducer` and `StringTokenizer`; it does not call `BaseSearcher`, `MatchesExtractor`, `grep-searcher`, `sinks::UTF8`, or `sinks::Lossy`.

Functional happy-path coverage exists for valid UTF-8:

- `search/searchers/tests_regular.rs`
- `search/searchers/tests_values.rs`
- `search/searchers/tests_linear.rs`
- `crates/core/text_grep/tests/grep_tests.rs`

These tests do not measure performance. There are no invalid-UTF-8 search tests and no `MatchesExtractor` tests.

## Agreed implementation phases

### Phase 1: baseline search benchmarks

Add valid-UTF-8 benchmarks before changing sink behavior. They should exercise processor-level search operations rather than session channels and UI orchestration.

Priority coverage:

- Regular search, including combined candidate matching and per-filter classification
- Value search, including capture extraction
- `MatchesExtractor`, including all capture processing
- `text_grep` only if consistency or public API coverage justifies it

Benchmark dimensions should include:

- Sparse and dense matches
- Different filter counts
- Broad and narrow regexes
- Representative line lengths and file sizes
- Setup/compilation separately from execution, or both search-only and end-to-end measurements
- Whole-file and incremental search where relevant

The benchmark must consume results so the optimizer cannot remove work. It should also avoid measuring repeated holder state incorrectly, because a successful `BaseSearcher` advances its internal byte and line positions.

### Phase 2: switch to lossy sink behavior

Replace strict search sinks with lossy behavior without introducing `RegexSet`. Keep the valid-UTF-8 benchmark inputs unchanged to obtain a direct before/after comparison.

Also add correctness coverage for:

- Invalid bytes on a non-matching line
- An invalid matching line
- Valid matches before and after an invalid matching line
- Regular search
- Value search
- Match extraction

Additional benchmarks can distinguish malformed UTF-8, Windows-1252-like bytes, UTF-16 with a BOM, and UTF-16 without a BOM. They should not be grouped under one generic “non-UTF-8” expectation because their matching behavior differs.

### Phase 3: optional `RegexSet` investigation

After baseline and lossy measurements, evaluate `RegexSet` only if the second regex phase is significant.

`RegexSet` is most relevant to regular search because it can return all matching filter indexes from one jointly compiled set. It does not return capture groups, so value search and extraction still require capture regexes. Possible comparisons are:

- Current combined grep matcher plus individual regex classification
- Combined grep matcher plus `RegexSet` classification
- A single decoded line-reading pipeline plus `RegexSet`, if replacing more of `grep-searcher` is justified

Do not introduce a custom matcher/sink side channel merely to transport pattern IDs; that would be brittle against `grep-searcher` internals. Any optimization must preserve multiple and overlapping filter matches.

## Deferred stream-reading issue

The separate log is:

```text
Producer Error: Data Source Error: Unrecoverable source error: Unable to decode input as UTF8
Unrecoverable error during producer session: Data Source Error: Unrecoverable source error: Unable to decode input as UTF8
```

This is not produced by the search sink.

The exact text comes from `tokio_util::codec::LinesCodec`. `ProcessSource` uses `LinesCodec` for process stdout and stderr in:

- `crates/core/sources/src/command/process.rs`

The flow is:

```text
process stdout/stderr
  -> LinesCodec requires UTF-8
  -> codec InvalidData error
  -> SourceError::Unrecoverable
  -> ProduceError::SourceError
  -> observing session logs the error and stops
```

This happens during ingestion, before content reaches a session file or any search API. Replacing `grep-searcher`'s `UTF8` sink with `Lossy` will not affect it.

The stream-reading issue will be investigated separately after the search work. It likely requires byte-oriented or explicitly lossy/encoding-aware line handling in the process source, but no design has been selected yet.

## Current status

- Research documented.
- No search implementation has been changed.
- No search benchmarks have been added.
- No user-warning behavior is planned.
- Search resilience work comes first.
- Process-stream decoding research comes later as a separate task.
