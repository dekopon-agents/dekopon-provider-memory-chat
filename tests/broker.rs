use std::path::{Path, PathBuf};

use dekopon_provider_sdk_testkit::{FakeBroker, StorageAccess, StorageInterface, StorageLimits};
use serde_json::{Value, json};

fn component() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("memory-chat-provider.wasm")
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

#[tokio::test(flavor = "multi_thread")]
async fn failed_second_append_rolls_back_the_whole_invocation() {
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
        failure.storage_evidence().is_some(),
        "the transaction started: {failure}"
    );
    let after = broker
        .invoke("memory.chat.recent", recent(1, 65_536))
        .await
        .expect("reads remain available after rollback");
    assert!(
        after["turns"].as_array().unwrap().is_empty(),
        "turn append was provisional"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn size_read_and_replace_host_failures_are_terminal_and_transactional() {
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
    assert!(replace_failure.storage_evidence().is_some());
    let after = replace_limited
        .invoke("memory.chat.recent", recent(1, 65_536))
        .await
        .expect("failed replacement rolls back both provisional appends");
    assert!(after["turns"].as_array().unwrap().is_empty());
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
    assert!(!Path::new("memory-chat-provider.wasm").is_dir());
}
