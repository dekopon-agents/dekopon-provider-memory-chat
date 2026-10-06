//! Durable, broker-curated chat memory with host-owned JSONL storage.
use clap::{Parser, Subcommand};
use dekopon_provider_sdk::provider::jsonl::StorageError;
use dekopon_provider_sdk::provider::{
    Capability, Code, Failure, Jsonl, Proposal, Provider, Stdout, Storage, Usage,
};
use dekopon_provider_sdk::{EffectKind, RiskLevel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fmt, io::Write};

const TURNS: &str = "turns.jsonl";
const CHUNK: u32 = 256 * 1024;

#[derive(Parser)]
#[command(name = "memory", about = "Read durable chat memory")]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Propose the newest N turns
    Recent {
        #[arg(long)]
        last: u64,
    },
    /// Propose a literal case-insensitive search
    Search {
        #[arg(long)]
        query: String,
    },
}

/// The broker-only delivered-turn route and two command-proposable reads.
pub struct MemoryChat;
/// Hidden delivered-turn recording capability.
pub struct Record;
/// Recent durable turns.
pub struct Recent;
/// Literal Unicode-lowercase search.
pub struct Search;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Turn {
    format: String,
    version: u8,
    id: String,
    commitment: String,
    user: String,
    assistant: String,
}

/// Broker-curated delivered turn; no command proposes this shape.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecordInput {
    operation: String,
    id: String,
    commitment: String,
    user: String,
    assistant: String,
    max_turn_bytes: u64,
    max_lookback_turns: u64,
    compaction_target_bytes: u64,
    compaction_threshold_bytes: u64,
}
/// Caller last plus broker-curated read bounds.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecentInput {
    last: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_lookback_turns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_recent_turns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_result_bytes: Option<u64>,
}
/// Caller query plus broker-curated search bounds.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchInput {
    query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_lookback_turns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_search_results: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_result_bytes: Option<u64>,
}

/// Stable provider failure with a broker-visible code.
#[derive(Debug)]
pub struct MemoryError {
    code: &'static str,
    message: &'static str,
}
impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}
impl Failure for MemoryError {
    fn code(&self) -> Code {
        Code::new(self.code)
    }
}
fn error(code: &'static str, message: &'static str) -> MemoryError {
    MemoryError { code, message }
}
fn corrupt() -> MemoryError {
    error("memory-corrupt", "chat memory is corrupt")
}
fn invalid() -> MemoryError {
    error("invalid-input", "memory input is invalid")
}
fn storage(failure: StorageError) -> MemoryError {
    match failure {
        StorageError::Corrupt => corrupt(),
        _ => error("storage-failed", "memory storage operation failed"),
    }
}

impl Provider for MemoryChat {
    const ID: &'static str = "memory-chat";
    const COMMAND_WORDS: &'static [&'static str] = &["memory"];
    const DESCRIPTION: &'static str = "Durable, on-demand, namespace-isolated chat memory";
    type Args = Args;
    type Capabilities = (Record, Recent, Search);
    fn propose(args: Args, _: bool) -> Result<Proposal<Self>, Usage> {
        match args.command {
            Command::Recent { last } if last > 0 => Ok(Proposal::to::<Recent>(RecentInput {
                last,
                operation: None,
                max_lookback_turns: None,
                max_recent_turns: None,
                max_result_bytes: None,
            })),
            Command::Search { query } if !query.is_empty() => {
                Ok(Proposal::to::<Search>(SearchInput {
                    query,
                    operation: None,
                    max_lookback_turns: None,
                    max_search_results: None,
                    max_result_bytes: None,
                }))
            }
            _ => Err(Usage::new(
                "supply a positive --last or nonempty --query TEXT",
            )),
        }
    }
}
impl Capability for Record {
    type Provider = MemoryChat;
    const NAME: &'static str = "record";
    const DESCRIPTION: &'static str = "Records one gateway-attested transport-accepted turn";
    const EFFECT: EffectKind = EffectKind::LocalWrite;
    const RISK: RiskLevel = RiskLevel::Medium;
    type Input = RecordInput;
    type Needs = Storage<Jsonl>;
    type Error = MemoryError;
    fn run(input: RecordInput, storage: Storage<Jsonl>, _: &mut Stdout) -> Result<(), MemoryError> {
        validate_record(&input)?;
        record(input, &storage)
    }
}
impl Capability for Recent {
    type Provider = MemoryChat;
    const NAME: &'static str = "recent";
    const DESCRIPTION: &'static str = "Returns recent durable turns in chronological order";
    const EFFECT: EffectKind = EffectKind::ReadOnly;
    const RISK: RiskLevel = RiskLevel::High;
    type Input = RecentInput;
    type Needs = Storage<Jsonl>;
    type Error = MemoryError;
    fn run(
        input: RecentInput,
        storage: Storage<Jsonl>,
        out: &mut Stdout,
    ) -> Result<(), MemoryError> {
        let (lookback, _, bytes) = validate_recent(&input)?;
        let turns = read_turns_tail(&storage, limit(lookback)?)?;
        emit(&bounded_result(&turns, limit(input.last)?, bytes)?, out)
    }
}
impl Capability for Search {
    type Provider = MemoryChat;
    const NAME: &'static str = "search";
    const DESCRIPTION: &'static str =
        "Searches recent durable turns with literal case-insensitive matching";
    const EFFECT: EffectKind = EffectKind::ReadOnly;
    const RISK: RiskLevel = RiskLevel::High;
    type Input = SearchInput;
    type Needs = Storage<Jsonl>;
    type Error = MemoryError;
    fn run(
        input: SearchInput,
        storage: Storage<Jsonl>,
        out: &mut Stdout,
    ) -> Result<(), MemoryError> {
        let (lookback, maximum, bytes) = validate_search(&input)?;
        let query = input.query.to_lowercase();
        let turns = read_turns_tail(&storage, limit(lookback)?)?;
        let matched = turns
            .iter()
            .filter(|turn| {
                turn.user.to_lowercase().contains(&query)
                    || turn.assistant.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        emit(&bounded_refs(&matched, limit(maximum)?, bytes)?, out)
    }
}
fn validate_record(input: &RecordInput) -> Result<(), MemoryError> {
    if input.operation != "record" {
        return Err(invalid());
    }
    Ok(())
}
fn validate_recent(input: &RecentInput) -> Result<(u64, u64, u64), MemoryError> {
    let (Some(operation), Some(lookback), Some(maximum), Some(bytes)) = (
        &input.operation,
        input.max_lookback_turns,
        input.max_recent_turns,
        input.max_result_bytes,
    ) else {
        return Err(invalid());
    };
    if operation != "recent" || input.last == 0 || input.last > maximum {
        return Err(invalid());
    }
    Ok((lookback, maximum, bytes))
}
fn validate_search(input: &SearchInput) -> Result<(u64, u64, u64), MemoryError> {
    let (Some(operation), Some(lookback), Some(maximum), Some(bytes)) = (
        &input.operation,
        input.max_lookback_turns,
        input.max_search_results,
        input.max_result_bytes,
    ) else {
        return Err(invalid());
    };
    if operation != "search" || input.query.is_empty() {
        return Err(invalid());
    }
    Ok((lookback, maximum, bytes))
}
fn limit(n: u64) -> Result<usize, MemoryError> {
    usize::try_from(n).map_err(|_| invalid())
}
fn emit(value: &Value, out: &mut impl Write) -> Result<(), MemoryError> {
    serde_json::to_writer(&mut *out, value)
        .map_err(|_| error("storage-failed", "stdout is closed"))?;
    out.write_all(b"\n")
        .map_err(|_| error("storage-failed", "stdout is closed"))
}
trait MemoryStore {
    fn size(&self, name: &str) -> Result<u64, StorageError>;
    fn read_chunk(
        &self,
        name: &str,
        offset: u64,
        length: u32,
    ) -> Result<dekopon_provider_sdk::provider::jsonl::Chunk, StorageError>;
    fn append(&self, name: &str, expected: u64, bytes: &[u8]) -> Result<u64, StorageError>;
    fn replace(&self, name: &str, expected: u64, bytes: &[u8]) -> Result<(), StorageError>;
}
impl MemoryStore for Storage<Jsonl> {
    fn size(&self, name: &str) -> Result<u64, StorageError> {
        self.size(name)
    }
    fn read_chunk(
        &self,
        name: &str,
        offset: u64,
        length: u32,
    ) -> Result<dekopon_provider_sdk::provider::jsonl::Chunk, StorageError> {
        self.read_chunk(name, offset, length)
    }
    fn append(&self, name: &str, expected: u64, bytes: &[u8]) -> Result<u64, StorageError> {
        self.append(name, expected, bytes)
    }
    fn replace(&self, name: &str, expected: u64, bytes: &[u8]) -> Result<(), StorageError> {
        self.replace(name, expected, bytes)
    }
}
fn record(input: RecordInput, storage_handle: &impl MemoryStore) -> Result<(), MemoryError> {
    let turn = Turn {
        format: "dekopon.chat-memory.turn".to_owned(),
        version: 1,
        id: input.id,
        commitment: input.commitment,
        user: input.user,
        assistant: input.assistant,
    };
    let turn_line = serde_json::to_vec(&turn).map_err(|_| corrupt())?;
    let canonical_turn_bytes = (turn_line.len() as u64)
        .checked_add(1)
        .ok_or_else(corrupt)?;
    if canonical_turn_bytes > input.max_turn_bytes {
        return Err(error(
            "result-too-large",
            "turn exceeds configured canonical line bound",
        ));
    }
    let (turns_size, turns_bytes) = read_file(storage_handle, TURNS)?;
    let mut turns = parse_lines::<Turn>(&turns_bytes, "dekopon.chat-memory.turn")?;
    let appended_size = storage_handle
        .append(TURNS, turns_size, &turn_line)
        .map_err(storage)?;
    turns.push(turn);
    if appended_size >= input.compaction_threshold_bytes {
        let compacted = compact(
            &turns,
            limit(input.max_lookback_turns)?,
            input.compaction_target_bytes,
        )?;
        storage_handle
            .replace(TURNS, appended_size, &compacted)
            .map_err(storage)?;
    }
    Ok(())
}
fn compact(turns: &[Turn], lookback: usize, target: u64) -> Result<Vec<u8>, MemoryError> {
    let mut selected = Vec::new();
    let mut bytes = 0_u64;
    for turn in turns.iter().rev().take(lookback) {
        let line = serde_json::to_vec(turn).map_err(|_| corrupt())?;
        let next = bytes
            .checked_add(line.len() as u64 + 1)
            .ok_or_else(corrupt)?;
        if next > target {
            if selected.is_empty() {
                return Err(error(
                    "result-too-large",
                    "newest turn cannot fit compaction target",
                ));
            }
            break;
        }
        selected.push(line);
        bytes = next;
    }
    selected.reverse();
    Ok(join_lines(&selected))
}
fn bounded_result(turns: &[Turn], maximum: usize, max_bytes: u64) -> Result<Value, MemoryError> {
    let refs = turns.iter().collect::<Vec<_>>();
    bounded_refs(&refs, maximum, max_bytes)
}
fn bounded_refs(turns: &[&Turn], maximum: usize, max_bytes: u64) -> Result<Value, MemoryError> {
    let candidates = turns
        .iter()
        .rev()
        .take(maximum)
        .copied()
        .collect::<Vec<_>>();
    let mut kept = Vec::new();
    for turn in candidates {
        kept.push(turn);
        let mut chronological = kept.clone();
        chronological.reverse();
        let value = json!({"turns": chronological, "truncated": kept.len() < turns.len().min(maximum) || turns.len() > maximum});
        let size = serde_json::to_vec(&value).map_err(|_| corrupt())?.len() as u64;
        if size > max_bytes {
            kept.pop();
            if kept.is_empty() {
                return Err(error(
                    "result-too-large",
                    "newest matching turn cannot fit result bound",
                ));
            }
            break;
        }
    }
    kept.reverse();
    let truncated = kept.len() < turns.len().min(maximum) || turns.len() > maximum;
    let result = json!({"turns": kept, "truncated": truncated});
    if serde_json::to_vec(&result).map_err(|_| corrupt())?.len() as u64 > max_bytes {
        return Err(error(
            "result-too-large",
            "memory result envelope cannot fit configured bound",
        ));
    }
    Ok(result)
}
fn read_turns_tail(handle: &impl MemoryStore, maximum: usize) -> Result<Vec<Turn>, MemoryError> {
    let size = match handle.size(TURNS) {
        Ok(size) => size,
        Err(StorageError::NotFound) => return Ok(Vec::new()),
        Err(error) => return Err(storage(error)),
    };
    let mut end = size;
    let mut bytes = Vec::new();
    while end > 0 && bytes.iter().filter(|byte| **byte == b'\n').count() <= maximum {
        let start = end.saturating_sub(u64::from(CHUNK));
        let length = u32::try_from(end - start).map_err(|_| corrupt())?;
        let chunk = handle.read_chunk(TURNS, start, length).map_err(storage)?;
        if chunk.next_offset != end || chunk.bytes.len() != length as usize {
            return Err(corrupt());
        }
        let mut combined = Vec::with_capacity(chunk.bytes.len().saturating_add(bytes.len()));
        combined.extend_from_slice(&chunk.bytes);
        combined.extend_from_slice(&bytes);
        bytes = combined;
        end = start;
    }
    if end > 0 {
        let boundary = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(corrupt)?;
        bytes.drain(..=boundary);
    }
    let mut turns = parse_lines(&bytes, "dekopon.chat-memory.turn")?;
    if turns.len() > maximum {
        turns.drain(..turns.len() - maximum);
    }
    Ok(turns)
}
fn read_file(handle: &impl MemoryStore, name: &str) -> Result<(u64, Vec<u8>), MemoryError> {
    let size = match handle.size(name) {
        Ok(size) => size,
        Err(StorageError::NotFound) => return Ok((0, Vec::new())),
        Err(error) => return Err(storage(error)),
    };
    let mut bytes = Vec::with_capacity(size.min(16 * 1024 * 1024) as usize);
    let mut offset = 0;
    while offset < size {
        let chunk = handle.read_chunk(name, offset, CHUNK).map_err(storage)?;
        if chunk.next_offset <= offset || chunk.next_offset > size {
            return Err(corrupt());
        }
        bytes.extend_from_slice(&chunk.bytes);
        offset = chunk.next_offset;
        if chunk.eof {
            break;
        }
    }
    if offset != size || bytes.len() as u64 != size {
        return Err(corrupt());
    }
    Ok((size, bytes))
}
fn parse_lines<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    expected: &str,
) -> Result<Vec<T>, MemoryError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.ends_with(b"\n") {
        return Err(corrupt());
    }
    bytes[..bytes.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| {
            let value: Value = serde_json::from_slice(line).map_err(|_| corrupt())?;
            if value.get("format").and_then(Value::as_str) != Some(expected)
                || value.get("version").and_then(Value::as_u64) != Some(1)
            {
                return Err(corrupt());
            }
            serde_json::from_value(value).map_err(|_| corrupt())
        })
        .collect()
}
fn join_lines(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut output = Vec::new();
    for line in lines {
        output.extend_from_slice(line);
        output.push(b'\n');
    }
    output
}

dekopon_provider_sdk::export!(MemoryChat);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broker_curated_inputs_are_required_and_operation_bound() {
        let recent = json!({"last":1,"operation":"recent","maxLookbackTurns":64,"maxRecentTurns":20,"maxResultBytes":65536});
        assert!(validate_recent(&serde_json::from_value(recent.clone()).unwrap()).is_ok());
        for bad in [
            json!({"last":1}),
            json!({"last":21,"operation":"recent","maxLookbackTurns":64,"maxRecentTurns":20,"maxResultBytes":65536}),
            json!({"last":1,"operation":"search","maxLookbackTurns":64,"maxRecentTurns":20,"maxResultBytes":65536}),
        ] {
            assert_eq!(
                validate_recent(&serde_json::from_value(bad).unwrap())
                    .unwrap_err()
                    .code,
                "invalid-input"
            );
        }
        let mut extra = recent;
        extra["untrusted"] = json!(true);
        assert!(serde_json::from_value::<RecentInput>(extra).is_err());
        let search = json!({"query":"Café","operation":"search","maxLookbackTurns":64,"maxSearchResults":20,"maxResultBytes":65536});
        assert!(validate_search(&serde_json::from_value(search.clone()).unwrap()).is_ok());
        let mut wrong = search;
        wrong["operation"] = json!("recent");
        assert_eq!(
            validate_search(&serde_json::from_value(wrong).unwrap())
                .unwrap_err()
                .code,
            "invalid-input"
        );
        assert!(serde_json::from_value::<RecordInput>(json!({"operation":"record"})).is_err());
    }

    #[test]
    fn result_bounds_and_literal_search_order() {
        let first = Turn {
            format: "dekopon.chat-memory.turn".into(),
            version: 1,
            id: "first".into(),
            commitment: "c".into(),
            user: "Café".into(),
            assistant: "answer".into(),
        };
        let second = Turn {
            id: "second".into(),
            user: "CAFÉ".into(),
            ..Turn {
                format: "dekopon.chat-memory.turn".into(),
                version: 1,
                id: "unused".into(),
                commitment: "c".into(),
                user: "unused".into(),
                assistant: "answer".into(),
            }
        };
        let turns = [&first, &second];
        let matched: Vec<_> = turns
            .into_iter()
            .filter(|turn| turn.user.to_lowercase().contains(&"café".to_lowercase()))
            .collect();
        assert_eq!(
            bounded_refs(&matched, 2, 65536).unwrap()["turns"][0]["id"],
            "first"
        );
        let one = bounded_refs(&matched, 1, 65536).unwrap();
        assert_eq!(one["turns"][0]["id"], "second");
        assert_eq!(one["truncated"], true);
        let exact = serde_json::to_vec(&one).unwrap().len() as u64;
        assert_eq!(bounded_refs(&matched, 1, exact).unwrap(), one);
        assert_eq!(
            bounded_refs(&matched, 1, exact - 1).unwrap_err().code,
            "result-too-large"
        );
        let empty = bounded_refs(&[], 1, 65536).unwrap();
        let bytes = serde_json::to_vec(&empty).unwrap().len() as u64;
        assert_eq!(
            bounded_refs(&[], 1, bytes - 1).unwrap_err().code,
            "result-too-large"
        );
    }

    struct FakeStore {
        data: std::cell::RefCell<Vec<u8>>,
        deny_append: bool,
        deny_replace: bool,
    }
    impl MemoryStore for FakeStore {
        fn size(&self, _: &str) -> Result<u64, StorageError> {
            Ok(self.data.borrow().len() as u64)
        }
        fn read_chunk(
            &self,
            _: &str,
            offset: u64,
            length: u32,
        ) -> Result<dekopon_provider_sdk::provider::jsonl::Chunk, StorageError> {
            let data = self.data.borrow();
            let start = offset as usize;
            let end = (start + length as usize).min(data.len());
            Ok(dekopon_provider_sdk::provider::jsonl::Chunk {
                bytes: data[start..end].to_vec(),
                next_offset: end as u64,
                eof: end == data.len(),
            })
        }
        fn append(&self, _: &str, expected: u64, bytes: &[u8]) -> Result<u64, StorageError> {
            if self.deny_append {
                return Err(StorageError::PermissionDenied);
            }
            let mut data = self.data.borrow_mut();
            if data.len() as u64 != expected {
                return Err(StorageError::InvalidArgument);
            }
            data.extend_from_slice(bytes);
            data.push(b'\n');
            Ok(data.len() as u64)
        }
        fn replace(&self, _: &str, expected: u64, bytes: &[u8]) -> Result<(), StorageError> {
            if self.deny_replace {
                return Err(StorageError::QuotaExceeded);
            }
            let mut data = self.data.borrow_mut();
            if data.len() as u64 != expected {
                return Err(StorageError::InvalidArgument);
            }
            *data = bytes.to_vec();
            Ok(())
        }
    }
    fn sample_record(id: &str, threshold: u64) -> RecordInput {
        RecordInput {
            operation: "record".into(),
            id: id.into(),
            commitment: "c".into(),
            user: "Café".into(),
            assistant: "answer".into(),
            max_turn_bytes: 4096,
            max_lookback_turns: 10,
            compaction_target_bytes: 4096,
            compaction_threshold_bytes: threshold,
        }
    }
    #[test]
    fn append_survives_refused_replace_and_read_only_denies_write() {
        let host = FakeStore {
            data: Default::default(),
            deny_append: false,
            deny_replace: true,
        };
        assert_eq!(
            record(sample_record("first", 1), &host).unwrap_err().code,
            "storage-failed"
        );
        assert_eq!(
            parse_lines::<Turn>(&host.data.borrow(), "dekopon.chat-memory.turn")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(read_turns_tail(&host, 1).unwrap()[0].id, "first");
        let readonly = FakeStore {
            data: Default::default(),
            deny_append: true,
            deny_replace: false,
        };
        assert_eq!(
            record(sample_record("denied", 4096), &readonly)
                .unwrap_err()
                .code,
            "storage-failed"
        );
        assert!(readonly.data.borrow().is_empty());
    }
    #[test]
    fn strict_lines_and_bounds() {
        let turn = Turn {
            format: "dekopon.chat-memory.turn".into(),
            version: 1,
            id: "one".into(),
            commitment: "c".into(),
            user: "Café".into(),
            assistant: "answer".into(),
        };
        let line = serde_json::to_vec(&turn).unwrap();
        assert_eq!(
            parse_lines::<Turn>(&join_lines(std::slice::from_ref(&line)), &turn.format)
                .unwrap()
                .len(),
            1
        );
        for bad in [b"{}\n".as_slice(), b"not-json\n", b"{}", b"\n"] {
            assert_eq!(
                parse_lines::<Turn>(bad, &turn.format).unwrap_err().code,
                "memory-corrupt"
            );
        }
        let exact = line.len() as u64 + 1;
        assert_eq!(
            compact(&[turn], 1, exact).unwrap(),
            join_lines(std::slice::from_ref(&line))
        );
        assert_eq!(
            compact(
                &parse_lines::<Turn>(
                    &join_lines(std::slice::from_ref(&line)),
                    "dekopon.chat-memory.turn"
                )
                .unwrap(),
                1,
                exact - 1
            )
            .unwrap_err()
            .code,
            "result-too-large"
        );
    }
}
