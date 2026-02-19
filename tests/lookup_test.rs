use extism::*;
use rs_plugin_common_interfaces::lookup::{RsLookupBook, RsLookupQuery, RsLookupWrapper};

fn build_plugin() -> Plugin {
    let wasm = Wasm::file("target/wasm32-unknown-unknown/release/rs_plugin_nh.wasm");
    let manifest = Manifest::new([wasm]).with_allowed_host("nhentai.net");
    Plugin::new(&manifest, [], true).expect("Failed to create plugin")
}

fn call_lookup(plugin: &mut Plugin, input: &RsLookupWrapper) -> serde_json::Value {
    let input_str = serde_json::to_string(input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup_metadata", &input_str)
        .expect("lookup_metadata call failed");
    serde_json::from_slice(output).expect("Failed to parse output JSON")
}

#[test]
fn test_lookup_empty_name_returns_404() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some(String::new()),
            ids: None,
        }),
        credential: None,
        params: None,
    };

    let input_str = serde_json::to_string(&input).unwrap();
    let error = plugin
        .call::<&str, &[u8]>("lookup_metadata", &input_str)
        .expect_err("Expected 404 error for empty search");

    let message = error.to_string();
    assert!(
        message.contains("Not supported") || message.contains("404"),
        "Expected error message to mention 404/Not supported, got: {message}"
    );
}

#[test]
fn test_lookup_exhibitionism_live_when_enabled() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    let results_array = results.as_array().expect("Expected an array");
    println!("Lookup results: {results_array:#?}");
    assert!(
        !results_array.is_empty(),
        "Expected at least one result for 'exhibitionism'"
    );

    let first = &results_array[0];
    assert!(
        first
            .get("metadata")
            .and_then(|m| m.get("book"))
            .and_then(|b| b.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false),
        "Expected a non-empty book name in the first result"
    );

    assert!(
        first
            .get("images")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "Expected at least one image in the first result"
    );
}

#[test]
fn test_lookup_direct_id_629637_live_when_enabled() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("nhentai:629637".to_string()),
            ids: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    let results_array = results.as_array().expect("Expected an array").clone();
    assert!(
        !results_array.is_empty(),
        "Expected at least one result for direct id nhentai:629637"
    );

    let first = &results_array[0];
    let book = first
        .get("metadata")
        .and_then(|m| m.get("book"))
        .expect("Expected book metadata");

    assert_eq!(
        book.get("id").and_then(|v| v.as_str()),
        Some("nhentai:629637"),
        "Expected direct-id lookup to preserve nhentai id"
    );

    assert_eq!(
        book.get("params")
            .and_then(|v| v.get("nhentaiId"))
            .and_then(|v| v.as_str()),
        Some("629637"),
        "Expected params.nhentaiId to match the requested id"
    );

    assert_eq!(
        book.get("params")
            .and_then(|v| v.get("nhentaiUrl"))
            .and_then(|v| v.as_str()),
        Some("https://nhentai.net/g/629637/"),
        "Expected params.nhentaiUrl to point to the direct gallery URL"
    );

    assert!(
        book.get("params")
            .and_then(|v| v.get("artists"))
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "Expected at least one artist extracted from the gallery page"
    );

    assert!(
        first
            .get("images")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "Expected at least one image in direct-id lookup result"
    );
    
}
