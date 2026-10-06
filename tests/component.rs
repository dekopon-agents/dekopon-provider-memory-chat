use dekopon_memory_chat_provider::MemoryChat;
use dekopon_provider_sdk::provider::{command, manifest};
use dekopon_provider_sdk_testkit::conformance;
use serde_json::json;

#[test]
fn built_component_conforms_to_manifest_stdio_and_empty_linker() {
    let component =
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT").expect("built component required");
    conformance::<MemoryChat>(component)
        .expect("JSONL and stdio only; empty linker refuses imports");
}

#[test]
fn exactly_three_roles_and_curated_schema() {
    let value = serde_json::to_value(manifest::<MemoryChat>().unwrap()).unwrap();
    assert_eq!(value["id"], "memory-chat");
    assert_eq!(value["commandWords"], json!(["memory"]));
    let capabilities = value["capabilities"].as_array().unwrap();
    assert_eq!(capabilities.len(), 3);
    for (cap, name, effect, risk) in [
        (&capabilities[0], "record", "local-write", "Medium"),
        (&capabilities[1], "recent", "read-only", "High"),
        (&capabilities[2], "search", "read-only", "High"),
    ] {
        assert_eq!(cap["id"], format!("memory-chat.{name}"));
        assert_eq!(cap["effect"], effect);
        assert_eq!(cap["risk"], risk);
        assert_eq!(cap["inputSchema"]["additionalProperties"], false);
    }
    let record = &capabilities[0]["inputSchema"];
    assert_eq!(record["required"].as_array().unwrap().len(), 9);
    for key in [
        "operation",
        "id",
        "commitment",
        "user",
        "assistant",
        "maxTurnBytes",
        "maxLookbackTurns",
        "compactionTargetBytes",
        "compactionThresholdBytes",
    ] {
        assert!(record["required"].as_array().unwrap().contains(&json!(key)));
    }
    for (cap, caller) in [(&capabilities[1], "last"), (&capabilities[2], "query")] {
        assert_eq!(cap["inputSchema"]["required"], json!([caller]));
        assert!(cap["inputSchema"]["properties"]["operation"].is_object());
        assert!(cap["inputSchema"]["properties"]["maxResultBytes"].is_object());
    }
}

#[test]
fn proposals_are_pure_and_record_is_unproposable() {
    for (argv, id, input) in [
        (
            vec!["recent", "--last", "3"],
            "memory-chat.recent",
            json!({"last":3}),
        ),
        (
            vec!["search", "--query", "Café"],
            "memory-chat.search",
            json!({"query":"Café"}),
        ),
    ] {
        let result = command::<MemoryChat>(
            &argv.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            false,
        );
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(result["capability"], id);
        assert_eq!(result["input"], input);
    }
    for argv in [
        vec!["record"],
        vec!["search", "-"],
        vec!["recent", "--last", "0"],
    ] {
        let result = command::<MemoryChat>(
            &argv.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            true,
        );
        let result = serde_json::to_value(result).unwrap();
        assert_ne!(result["outcome"], "proposed", "{argv:?}");
    }
}
