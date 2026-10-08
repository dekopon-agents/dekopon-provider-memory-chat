//! Real published 0.36 broker + storage host JSONL witnesses against the built component.
//!
//! Read-only write-denial hosting gap: broker 0.36 enforces read-write JSONL for the
//! record route and read-only JSONL for recent/search when constructing its routed
//! constraint catalog (`CapabilityRoute::chat_memory_access`, `ConstraintCatalog::new`).
//! The real recent/search implementations call only size/read_chunk; no caller input
//! can make either attempt append/replace. Record is hidden behind delivered-turn
//! curation and cannot be invoked under a read-only grant. Thus no invocation of
//! this unchanged component through the published broker can issue a write under a
//! read-only grant. The injected-storage native denial witness stays in
//! `src/lib.rs::tests::append_survives_refused_replace_and_read_only_denies_write`.
#![cfg(unix)]
use dekopon_broker::{
    Attestation, AttestorGrant, Broker, BrokerLimits, CapabilityRoute, ChatMemoryConfig,
    ChatTransportKind, ConstraintCatalog, ConstraintSet, Conversation, ConversationKind,
    CredentialStore, IdentityDirectory, InMemoryAuditLog, PolicyEngine, PolicyWorld,
};
use dekopon_broker_host::{
    BrokerHostLimits, BrokerProviderRegistry, CommandRunOutcome, Streams, asset::AssetInputs,
};
use dekopon_broker_protocol::{
    ChatScopeClaim, DeliveredAnswer, DeliveredTurnRequest, DeliveryIdentity, InvocationRequest,
};
use dekopon_capability::{
    EffectKind, ExecutionConstraints, InvocationOutcome, StorageAccess, StorageConstraints,
    StorageInterface, StorageScope,
};
use dekopon_core::{Actor, RiskLevel};
use dekopon_storage_host::{ContinuityPolicy, StorageHost, StorageLimits};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

const TRACE: &str = "00-0000000000000000000000000000f1c7-00000000000000f1-00";

fn stdout_streams() -> (AssetInputs, std::thread::JoinHandle<Vec<u8>>) {
    let (writer, mut reader) = std::os::unix::net::UnixStream::pair().unwrap();
    let captured = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut bytes).unwrap();
        bytes
    });
    (
        AssetInputs {
            streams: Some(Streams {
                stdin: None,
                stdout: writer.into(),
            }),
            ..Default::default()
        },
        captured,
    )
}
fn constraint(
    route: CapabilityRoute,
    effect: EffectKind,
    risk: RiskLevel,
    access: StorageAccess,
) -> ConstraintSet {
    ConstraintSet {
        route,
        provider: "memory-chat".parse().unwrap(),
        effect,
        risk,
        credential: None,
        constraints: ExecutionConstraints {
            asset: None,
            timeout_ms: 10_000,
            http: None,
            secret_use: None,
            storage: Some(StorageConstraints {
                interface: StorageInterface::Jsonl,
                access,
                scope: StorageScope::PrivateConversation,
                retention: Default::default(),
            }),
        },
    }
}
async fn broker(root: &Path) -> Broker<InMemoryAuditLog> {
    let storage = StorageHost::open(root, StorageLimits::default()).unwrap();
    let registry = BrokerProviderRegistry::load_with_storage(
        [std::path::PathBuf::from(
            std::env::var_os("DEKOPON_PROVIDER_COMPONENT").expect("built component"),
        )],
        BrokerHostLimits::default(),
        Some(storage),
    )
    .await
    .unwrap();
    let world = PolicyWorld::new(
        ["gateway".parse().unwrap(), "maintainer".parse().unwrap()],
        registry
            .capabilities()
            .map(|(provider, capability)| (capability.id.clone(), provider.clone())),
    )
    .unwrap();
    let policy = PolicyEngine::new(r#"
        @id("prompt") permit(principal == Dekopon::Principal::"maintainer",
            action == Dekopon::Action::"agent.prompt", resource == Dekopon::Agent::"reviewer")
            when { context.via == "gateway" };
        @id("record") permit(principal == Dekopon::Principal::"maintainer",
            action == Dekopon::Action::"memory-chat.record", resource == Dekopon::Provider::"memory-chat")
            when { context.via == "gateway" && context.agent == "reviewer" };
        @id("recent") permit(principal == Dekopon::Principal::"maintainer",
            action == Dekopon::Action::"memory-chat.recent", resource == Dekopon::Provider::"memory-chat")
            when { context.via == "gateway" && context.agent == "reviewer" };
        @id("search") permit(principal == Dekopon::Principal::"maintainer",
            action == Dekopon::Action::"memory-chat.search", resource == Dekopon::Provider::"memory-chat")
            when { context.via == "gateway" && context.agent == "reviewer" };"#, &world).unwrap();
    let constraints = ConstraintCatalog::new(
        [
            (
                "memory-chat.record",
                CapabilityRoute::ChatMemoryRecord,
                EffectKind::LocalWrite,
                RiskLevel::Medium,
                StorageAccess::ReadWrite,
            ),
            (
                "memory-chat.recent",
                CapabilityRoute::ChatMemoryRecent,
                EffectKind::ReadOnly,
                RiskLevel::High,
                StorageAccess::ReadOnly,
            ),
            (
                "memory-chat.search",
                CapabilityRoute::ChatMemorySearch,
                EffectKind::ReadOnly,
                RiskLevel::High,
                StorageAccess::ReadOnly,
            ),
        ]
        .map(|(name, route, effect, risk, access)| {
            (
                name.parse().unwrap(),
                constraint(route, effect, risk, access),
            )
        }),
    )
    .unwrap();
    Broker::new(
        registry,
        "broker".parse().unwrap(),
        "memory-test".to_owned(),
        policy,
        constraints,
        CredentialStore::empty(),
        IdentityDirectory::new([(
            "slack.t0123abc.u9xyz".parse().unwrap(),
            "maintainer".parse().unwrap(),
        )])
        .unwrap(),
        Arc::new(InMemoryAuditLog::new(64).unwrap()),
        BrokerLimits::default(),
    )
    .unwrap()
    .with_chat_memory(ChatMemoryConfig {
        continuity_policy: ContinuityPolicy::AuthorityBound,
        enabled_agents: vec!["reviewer".parse().unwrap()],
        max_lookback_turns: 200,
        max_recent_turns: 20,
        max_search_results: 20,
        max_query_bytes: 256,
        max_result_bytes: 65536,
        max_turn_bytes: 32768,
        compaction_target_bytes: 8388608,
        compaction_threshold_bytes: 12582912,
    })
    .unwrap()
}
fn peer() -> dekopon_broker::AuthenticatedContext {
    dekopon_broker::AuthenticatedContext::new(
        "gateway".parse().unwrap(),
        Actor::Service {
            principal: "gateway".parse().unwrap(),
        },
    )
    .unwrap()
}
fn claim(conversation: &str) -> Attestation {
    let (channel, timestamp) = conversation.split_once(':').unwrap();
    Attestation::for_chat(
        "slack.t0123abc.u9xyz".parse().unwrap(),
        "reviewer".parse().unwrap(),
        ChatScopeClaim {
            transport: "scientist-slack".parse().unwrap(),
            kind: ChatTransportKind::Slack,
            conversation: Conversation {
                kind: ConversationKind::Thread,
                container: Some("t0123abc".into()),
                id: channel.into(),
                thread: Some(timestamp.into()),
            },
            trigger: dekopon_broker::Trigger::Message,
        },
    )
}
fn grant() -> AttestorGrant {
    AttestorGrant {
        namespaces: Some(vec!["slack.t0123abc".into()]),
    }
}
fn temp_root() -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "memory-chat-broker-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root.canonicalize().unwrap()
}
async fn record(broker: &Broker<InMemoryAuditLog>, conversation: &str, id: &str, marker: &str) {
    let claim = claim(conversation);
    let request = DeliveredTurnRequest::new(
        id.parse().unwrap(),
        TRACE.parse().unwrap(),
        DeliveryIdentity::Slack {
            channel: conversation.split_once(':').unwrap().0.into(),
            timestamp: conversation.split_once(':').unwrap().1.into(),
        },
        marker.to_owned(),
        DeliveredAnswer::accepted_by_transport("assistant".into()),
    );
    let result = broker
        .record_delivered_turn(
            &peer(),
            Some(&grant()),
            &claim.bound_to(id.parse().unwrap()),
            request,
        )
        .await
        .unwrap();
    assert_eq!(result.outcome, InvocationOutcome::Succeeded, "{result:?}");
}
async fn invoke_read(
    broker: &Broker<InMemoryAuditLog>,
    conversation: &str,
    argv: &[&str],
    id: &str,
) -> (dekopon_broker_protocol::InvocationResult, Vec<u8>) {
    let claim = claim(conversation);
    let args = argv.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let proposed = broker
        .run_command(
            &peer(),
            Some(&grant()),
            Some(&claim),
            "memory",
            &args,
            false,
        )
        .await
        .unwrap();
    let CommandRunOutcome::Proposed {
        capability, input, ..
    } = proposed
    else {
        panic!("read must propose");
    };
    let (streams, capture) = stdout_streams();
    let result = broker
        .invoke(
            &peer(),
            Some(&grant()),
            Some(&claim.bound_to(id.parse().unwrap())),
            InvocationRequest {
                id: id.parse().unwrap(),
                capability,
                input,
                trace_parent: TRACE.parse().unwrap(),
                secret_use: None,
            },
            streams,
        )
        .await
        .unwrap();
    let bytes = capture.join().unwrap();
    (result.result, bytes)
}
async fn read(
    broker: &Broker<InMemoryAuditLog>,
    conversation: &str,
    argv: &[&str],
    id: &str,
) -> Value {
    let (result, bytes) = invoke_read(broker, conversation, argv, id).await;
    assert_eq!(result.outcome, InvocationOutcome::Succeeded, "{result:?}");
    assert_eq!(bytes.iter().filter(|&&b| b == b'\n').count(), 1);
    serde_json::from_slice(&bytes).unwrap()
}
fn stored_log(root: &Path) -> std::path::PathBuf {
    fn visit(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, found);
            } else if std::fs::read(&path)
                .unwrap()
                .windows(b"dekopon.chat-memory.turn".len())
                .any(|w| w == b"dekopon.chat-memory.turn")
            {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    visit(root, &mut found);
    assert_eq!(found.len(), 1);
    found.pop().unwrap()
}
#[tokio::test(flavor = "multi_thread")]
async fn published_broker_records_reads_searches_and_isolates_namespace() {
    let root = temp_root();
    let host = broker(&root.join("storage")).await;
    record(
        &host,
        "c0123abc:1712345678.000100",
        "turn-one",
        "Café marker",
    )
    .await;
    let recent = read(
        &host,
        "c0123abc:1712345678.000100",
        &["recent", "--last", "1"],
        "read-one",
    )
    .await;
    assert_eq!(recent["turns"][0]["user"], "Café marker");
    let search = read(
        &host,
        "c0123abc:1712345678.000100",
        &["search", "--query", "café"],
        "search-one",
    )
    .await;
    assert_eq!(search["turns"][0]["user"], "Café marker");
    let isolated = read(
        &host,
        "cother:1712345678.000200",
        &["recent", "--last", "1"],
        "read-other",
    )
    .await;
    assert_eq!(isolated["turns"], json!([]));
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test(flavor = "multi_thread")]
async fn published_broker_bounds_ordered_whole_turns() {
    let root = temp_root();
    let host = broker(&root.join("storage")).await;
    for (id, marker) in [
        ("turn-a", "alpha"),
        ("turn-b", "bravo"),
        ("turn-c", "charlie"),
    ] {
        record(
            &host,
            "c0123abc:1712345678.000100",
            id,
            &format!("{marker} {}", "x".repeat(26_000)),
        )
        .await;
    }
    let result = read(
        &host,
        "c0123abc:1712345678.000100",
        &["recent", "--last", "3"],
        "read-bounded",
    )
    .await;
    assert_eq!(result["truncated"], true);
    let turns = result["turns"].as_array().unwrap();
    assert!(turns.len() < 3);
    assert_eq!(
        turns.last().unwrap()["user"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .next(),
        Some("charlie")
    );
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test(flavor = "multi_thread")]
async fn published_broker_refuses_corrupt_and_truncated_log_lines() {
    for (index, fixture) in [
        b"not-json\n".as_slice(),
        b"{\"format\":\"dekopon.chat-memory.turn\",\"version\":1}".as_slice(),
        b"{\"format\":\"dekopon.chat-memory.turn\",\"version\":2}\n".as_slice(),
        b"{\"format\":\"dekopon.chat-memory.turn\",\"version\":1,\"id\":\"x\",\"commitment\":\"c\",\"user\":\"u\",\"assistant\":\"a\",\"extra\":1}\n".as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        let root = temp_root();
        let host = broker(&root.join("storage")).await;
        record(&host, "c0123abc:1712345678.000100", "seed", "seed").await;
        std::fs::write(stored_log(&root), fixture).unwrap();
        let (result, bytes) = invoke_read(
            &host,
            "c0123abc:1712345678.000100",
            &["recent", "--last", "1"],
            &format!("corrupt-{index}"),
        )
        .await;
        assert_eq!(result.outcome, InvocationOutcome::Failed, "{result:?}");
        assert!(bytes.is_empty());
        drop(host);
        std::fs::remove_dir_all(root).unwrap();
    }
}
