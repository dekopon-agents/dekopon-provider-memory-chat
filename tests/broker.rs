use std::{
    fs,
    path::{Path, PathBuf},
};

use dekopon_provider_sdk_testkit::{
    CommandRunOutcome, FakeBroker, FakeBrokerError, StorageAccess, StorageInterface, StorageLimits,
};
use serde_json::{Value, json};

fn component() -> PathBuf {
    PathBuf::from(
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must point at the built component"),
    )
}

fn stored_turns_path(root: &Path) -> PathBuf {
    fn visit(directory: &Path, matches: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory).expect("walk storage root") {
            let path = entry.expect("storage entry").path();
            if path.is_dir() {
                visit(&path, matches);
            } else if path.parent().and_then(Path::file_name) == Some("data".as_ref())
                && fs::read(&path)
                    .expect("read opaque logical file")
                    .windows(b"dekopon.chat-memory.turn".len())
                    .any(|window| window == b"dekopon.chat-memory.turn")
            {
                matches.push(path);
            }
        }
    }

    let mut matches = Vec::new();
    visit(root, &mut matches);
    assert_eq!(matches.len(), 1, "exactly one turns log must exist");
    matches.pop().expect("turns log")
}

fn canonical_turn_bytes(id: &str, commitment: &str, user: &str, assistant: &str) -> u64 {
    format!(
        "{{\"format\":\"dekopon.chat-memory.turn\",\"version\":1,\"id\":{},\"commitment\":{},\"user\":{},\"assistant\":{}}}",
        serde_json::to_string(id).expect("id JSON"),
        serde_json::to_string(commitment).expect("commitment JSON"),
        serde_json::to_string(user).expect("user JSON"),
        serde_json::to_string(assistant).expect("assistant JSON")
    )
    .len() as u64
        + 1
}

fn canonical_dedup_bytes(id: &str, commitment: &str) -> u64 {
    format!(
        "{{\"format\":\"dekopon.chat-memory.dedup\",\"version\":1,\"id\":{},\"commitment\":{}}}",
        serde_json::to_string(id).expect("id JSON"),
        serde_json::to_string(commitment).expect("commitment JSON")
    )
    .len() as u64
        + 1
}

fn record(id: &str, commitment: &str, user: &str, assistant: &str) -> Value {
    json!({
        "operation": "record",
        "id": id,
        "commitment": commitment,
        "user": user,
        "assistant": assistant,
        "maxTurnBytes": 4096,
        "maxLookbackTurns": 64,
        "maxDedupRecords": 64,
        "maxDedupBytes": 65536,
        "compactionTargetBytes": 8192,
        "compactionThresholdBytes": 16384
    })
}

fn recent(last: u64, max_result_bytes: u64) -> Value {
    json!({
        "operation": "recent",
        "last": last,
        "maxLookbackTurns": 64,
        "maxRecentTurns": 64,
        "maxResultBytes": max_result_bytes
    })
}

fn search(query: &str, maximum: u32, max_result_bytes: u64) -> Value {
    json!({
        "operation": "search",
        "query": query,
        "maxLookbackTurns": 64,
        "maxSearchResults": maximum,
        "maxResultBytes": max_result_bytes
    })
}

/// Whether the failure carries the storage evidence a namespace-touching invocation records.
///
/// `FakeBrokerError::storage_evidence` was deleted from the testkit in 0.13.0 as unreferenced
/// public surface; the field it read is still public, so this repository keeps the one line it
/// needs rather than pinning an older testkit.
fn storage_was_reached(error: &FakeBrokerError) -> bool {
    matches!(error, FakeBrokerError::Invocation(failure) if failure.storage.is_some())
}

async fn run(broker: &FakeBroker, argv: &[&str], stdin: Option<&str>) -> Value {
    let argv = argv
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<Vec<_>>();
    let outcome: CommandRunOutcome = broker
        .run_command("memory", &argv, stdin)
        .await
        .expect("the memory word runs through the component");
    serde_json::to_value(outcome).expect("command run JSON")
}

async fn broker() -> FakeBroker {
    FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadWrite)
        .build()
        .await
        .expect("memory-chat loads with exactly JSONL storage")
}

#[tokio::test(flavor = "multi_thread")]
async fn checked_component_manifest_and_command_runs_are_exact() {
    let broker = broker().await;
    let manifests = broker.registry().manifests().collect::<Vec<_>>();
    assert_eq!(manifests.len(), 1);
    assert_eq!(
        serde_json::to_value(manifests[0]).expect("manifest JSON"),
        json!({
            "apiVersion": "dekopon.dev/provider/v1alpha1",
            "id": "memory-chat",
            "description": "Durable, on-demand, namespace-isolated chat memory",
            "commandWords": ["memory"],
            "capabilities": [
                {
                    "id": "memory.chat.record",
                    "description": "Records one gateway-attested transport-accepted turn",
                    "effect": "local-write",
                    "risk": "Medium",
                    "inputSchema": {"type":"object","additionalProperties":false}
                },
                {
                    "id": "memory.chat.recent",
                    "description": "Returns recent durable turns in chronological order",
                    "effect": "read-only",
                    "risk": "High",
                    "inputSchema": {
                        "type":"object",
                        "properties":{"last":{"type":"integer","minimum":1}},
                        "required":["last"],
                        "additionalProperties":false
                    }
                },
                {
                    "id": "memory.chat.search",
                    "description": "Searches recent durable turns with literal case-insensitive matching",
                    "effect": "read-only",
                    "risk": "High",
                    "inputSchema": {
                        "type":"object",
                        "properties":{"query":{"type":"string","minLength":1}},
                        "required":["query"],
                        "additionalProperties":false
                    }
                }
            ]
        })
    );

    let recent = run(&broker, &["recent", "--last", "3"], None).await;
    assert_eq!(
        recent,
        json!({
            "outcome": "proposed",
            "capability": "memory.chat.recent",
            "input": {"last": 3}
        })
    );

    let search = run(&broker, &["search", "--query", "needle"], None).await;
    assert_eq!(
        search,
        json!({
            "outcome": "proposed",
            "capability": "memory.chat.search",
            "input": {"query": "needle"}
        })
    );

    // The host carries the piped value only for a `run-command` guest; a legacy rewrite never
    // received one. Reaching the capability through it is what proves the new export is live.
    let piped = run(&broker, &["search", "-"], Some(" piped needle \n")).await;
    assert_eq!(
        piped,
        json!({
            "outcome": "proposed",
            "capability": "memory.chat.search",
            "input": {"query": "piped needle"}
        })
    );

    let help = run(&broker, &["--help"], None).await;
    assert_eq!(help["outcome"], "rendered");
    assert_eq!(help["status"], 0);
    assert_eq!(help["stderr"], "");
    let page = help["stdout"].as_str().expect("help page");
    assert!(
        page.starts_with("Usage: memory recent --last N\n"),
        "{page}"
    );
    assert!(page.contains("memory search -\n"), "{page}");

    for (argv, stdin, stderr) in [
        (
            &["record"][..],
            None,
            "memory: unrecognized arguments; try `memory --help`\n",
        ),
        (
            &["search", "-"][..],
            None,
            "memory search -: nothing was piped in\n",
        ),
    ] {
        assert_eq!(
            run(&broker, argv, stdin).await,
            json!({"outcome": "rendered", "stdout": "", "stderr": stderr, "status": 2}),
            "{argv:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn durable_record_recent_search_dedup_and_conflict_are_exact() {
    let broker = broker().await;
    assert_eq!(
        broker
            .invoke(
                "memory.chat.record",
                record("turn-1", "commitment-1", "Café question", "First answer"),
            )
            .await
            .expect("first record"),
        json!({"recorded": true, "duplicate": false})
    );
    broker
        .invoke(
            "memory.chat.record",
            record("turn-2", "commitment-2", "second question", "CAFÉ reply"),
        )
        .await
        .expect("second record");

    let recent = broker
        .invoke("memory.chat.recent", recent(2, 65_536))
        .await
        .expect("later invocation reads durable turns");
    assert_eq!(recent["turns"][0]["id"], "turn-1");
    assert_eq!(recent["turns"][1]["id"], "turn-2");
    assert_eq!(recent["truncated"], false);

    let matched = broker
        .invoke("memory.chat.search", search("café", 1, 65_536))
        .await
        .expect("Unicode-lowercase literal search");
    assert_eq!(matched["turns"].as_array().unwrap().len(), 1);
    assert_eq!(matched["turns"][0]["id"], "turn-2");
    assert_eq!(matched["truncated"], true);

    assert_eq!(
        broker
            .invoke(
                "memory.chat.record",
                record("turn-2", "commitment-2", "ignored", "ignored"),
            )
            .await
            .expect("same identity and commitment is idempotent"),
        json!({"recorded": false, "duplicate": true})
    );
    let conflict = broker
        .invoke(
            "memory.chat.record",
            record("turn-2", "changed", "ignored", "ignored"),
        )
        .await
        .expect_err("same identity with changed commitment conflicts");
    assert_eq!(
        conflict.provider_failure().map(|value| value.0),
        Some("dedup-conflict")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn namespaces_and_storage_access_are_host_owned() {
    let first = broker().await;
    first
        .invoke(
            "memory.chat.record",
            record("private", "private-c", "secret user", "secret answer"),
        )
        .await
        .expect("record in first namespace");

    let second = FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadWrite)
        .subject("slack.t0123abc.udifferent")
        .build()
        .await
        .expect("second namespace loads");
    let isolated = second
        .invoke("memory.chat.recent", recent(1, 65_536))
        .await
        .expect("isolated namespace reads cleanly");
    assert!(isolated["turns"].as_array().unwrap().is_empty());

    let read_only = FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadOnly)
        .build()
        .await
        .expect("read-only linker still loads");
    let denied = read_only
        .invoke(
            "memory.chat.record",
            record("denied", "denied-c", "user", "assistant"),
        )
        .await
        .expect_err("host refuses append without write authority");
    assert!(
        denied.provider_failure().is_none(),
        "host refusal is not guest policy: {denied}"
    );
}

/// Dekopon 0.13.0 applies each storage call directly and has no invocation rollback, so a record
/// whose dedup append is refused keeps the turn append that already completed. The provider is
/// still fail-closed on the operation — it reports the refusal and writes no dedup line — but the
/// turn log is not restored, and a later `recent` sees it.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_second_append_keeps_the_completed_first_one() {
    let limits = StorageLimits {
        max_write_bytes_per_call: 220,
        max_write_bytes_per_invocation: 220,
        ..StorageLimits::default()
    };
    let broker = FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadWrite)
        .storage_limits(limits)
        .build()
        .await
        .expect("narrow valid storage limits");

    let failure = broker
        .invoke(
            "memory.chat.record",
            record("rollback", "rollback-c", "1234567890", "abcdefghij"),
        )
        .await
        .expect_err("cumulative quota rejects the later dedup append");
    assert!(
        storage_was_reached(&failure),
        "the namespace was reached: {failure}"
    );
    let after = broker
        .invoke("memory.chat.recent", recent(1, 65_536))
        .await
        .expect("reads remain available after the refusal");
    assert_eq!(after["turns"].as_array().unwrap().len(), 1);
    assert_eq!(after["turns"][0]["id"], "rollback");
}

#[tokio::test(flavor = "multi_thread")]
async fn size_read_and_replace_host_failures_are_terminal() {
    let size_limited = FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadWrite)
        .storage_limits(StorageLimits {
            max_host_calls_per_invocation: 1,
            ..StorageLimits::default()
        })
        .build()
        .await
        .expect("one-call storage profile is valid");
    let size_failure = size_limited
        .invoke(
            "memory.chat.record",
            record("size-failure", "size-c", "user", "assistant"),
        )
        .await
        .expect_err("the second size operation exceeds the host-call ceiling");
    assert!(
        size_failure.provider_failure().is_none(),
        "size refusal belongs to the host: {size_failure}"
    );

    let read_limited = FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadWrite)
        .storage_limits(StorageLimits {
            max_read_bytes_per_call: 1,
            max_read_bytes_per_invocation: 1,
            ..StorageLimits::default()
        })
        .build()
        .await
        .expect("narrow read profile is valid");
    read_limited
        .invoke(
            "memory.chat.record",
            record("read-failure", "read-c", "user", "assistant"),
        )
        .await
        .expect("an empty namespace records without read-chunk");
    let read_failure = read_limited
        .invoke("memory.chat.recent", recent(1, 65_536))
        .await
        .expect_err("fixed 256 KiB guest read exceeds the narrow host call limit");
    assert!(
        read_failure.provider_failure().is_none(),
        "read refusal belongs to the host: {read_failure}"
    );

    let replace_limited = FakeBroker::builder()
        .component(component())
        .provider("memory-chat")
        .storage(StorageInterface::Jsonl, StorageAccess::ReadWrite)
        .storage_limits(StorageLimits {
            max_write_bytes_per_call: 300,
            max_write_bytes_per_invocation: 300,
            ..StorageLimits::default()
        })
        .build()
        .await
        .expect("replace-failure profile is valid");
    let mut compacting = record("replace-failure", "replace-c", "user", "assistant");
    compacting["compactionThresholdBytes"] = json!(1);
    compacting["compactionTargetBytes"] = json!(4096);
    let replace_failure = replace_limited
        .invoke("memory.chat.record", compacting)
        .await
        .expect_err("replacement exceeds cumulative write quota after both appends");
    assert!(storage_was_reached(&replace_failure));
    let after = replace_limited
        .invoke("memory.chat.recent", recent(1, 65_536))
        .await
        .expect("reads remain available after the refused replacement");
    assert_eq!(after["turns"].as_array().unwrap().len(), 1);
    assert_eq!(after["turns"][0]["id"], "replace-failure");
}

#[tokio::test(flavor = "multi_thread")]
async fn canonical_turn_and_dedup_byte_bounds_accept_exactly_and_reject_one_less() {
    let exact_turn = broker().await;
    let turn_bytes = canonical_turn_bytes("turn-bound", "turn-c", "user", "assistant");
    let mut input = record("turn-bound", "turn-c", "user", "assistant");
    input["maxTurnBytes"] = json!(turn_bytes);
    exact_turn
        .invoke("memory.chat.record", input)
        .await
        .expect("exact maxTurnBytes accepts the complete canonical line");

    let short_turn = broker().await;
    let mut input = record("turn-bound", "turn-c", "user", "assistant");
    input["maxTurnBytes"] = json!(turn_bytes - 1);
    let error = short_turn
        .invoke("memory.chat.record", input)
        .await
        .expect_err("one byte below maxTurnBytes is rejected");
    assert_eq!(
        error.provider_failure().map(|value| value.0),
        Some("result-too-large")
    );

    let exact_dedup = broker().await;
    let dedup_bytes = canonical_dedup_bytes("dedup-bound", "dedup-c");
    let mut input = record("dedup-bound", "dedup-c", "user", "assistant");
    input["maxDedupBytes"] = json!(dedup_bytes);
    exact_dedup
        .invoke("memory.chat.record", input)
        .await
        .expect("exact maxDedupBytes accepts the complete canonical line");

    let short_dedup = broker().await;
    let mut input = record("dedup-bound", "dedup-c", "user", "assistant");
    input["maxDedupBytes"] = json!(dedup_bytes - 1);
    let error = short_dedup
        .invoke("memory.chat.record", input)
        .await
        .expect_err("one byte below maxDedupBytes is rejected");
    assert_eq!(
        error.provider_failure().map(|value| value.0),
        Some("dedup-capacity")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_and_truncated_host_backed_turn_logs_fail_closed() {
    let fixtures = [
        br#"{"format":"dekopon.chat-memory.turn","version":2,"id":"bad","commitment":"c","user":"u","assistant":"a"}
"#.as_slice(),
        br#"{"format":"dekopon.chat-memory.turn","version":1,"id":"bad","commitment":"c","user":"u","assistant":"a","unknown":true}
"#.as_slice(),
        b"{\"format\":\n".as_slice(),
        br#"{"format":"dekopon.chat-memory.turn","version":1,"id":"bad","commitment":"c","user":"u","assistant":"a"}"#.as_slice(),
    ];

    for (index, fixture) in fixtures.into_iter().enumerate() {
        let broker = broker().await;
        broker
            .invoke(
                "memory.chat.record",
                record("seed", "seed-c", "user", "assistant"),
            )
            .await
            .expect("seed host-backed turns log");
        fs::write(stored_turns_path(broker.storage_root()), fixture)
            .expect("install corrupt host-backed fixture");
        let error = broker
            .invoke("memory.chat.recent", recent(1, 65_536))
            .await
            .expect_err("corrupt host-backed turns log must fail closed");
        assert_eq!(
            error.provider_failure().map(|value| value.0),
            Some("memory-corrupt"),
            "fixture {index}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn compaction_dedup_capacity_and_result_bounds_remain_independent() {
    let broker = broker().await;
    for index in 0..3 {
        let mut value = record(
            &format!("compact-{index}"),
            &format!("commit-{index}"),
            &format!("user-{index}"),
            &format!("answer-{index}"),
        );
        value["compactionThresholdBytes"] = json!(1);
        value["compactionTargetBytes"] = json!(420);
        value["maxLookbackTurns"] = json!(2);
        broker
            .invoke("memory.chat.record", value)
            .await
            .expect("bounded compaction succeeds");
    }
    let compacted = broker
        .invoke("memory.chat.recent", recent(3, 65_536))
        .await
        .expect("compacted turns remain readable");
    assert!(compacted["turns"].as_array().unwrap().len() <= 2);
    assert_eq!(
        compacted["turns"].as_array().unwrap().last().unwrap()["id"],
        "compact-2"
    );

    let mut capped = record("capacity", "capacity-c", "u", "a");
    capped["maxDedupRecords"] = json!(3);
    let error = broker
        .invoke("memory.chat.record", capped)
        .await
        .expect_err("permanent dedup log capacity is enforced despite turn compaction");
    assert_eq!(
        error.provider_failure().map(|value| value.0),
        Some("dedup-capacity")
    );

    let too_small = broker
        .invoke("memory.chat.recent", recent(1, 1))
        .await
        .expect_err("even envelope must fit result bound");
    assert_eq!(
        too_small.provider_failure().map(|value| value.0),
        Some("result-too-large")
    );
}

#[test]
fn generated_component_is_not_a_source_fixture() {
    assert!(!component().is_dir());
}
