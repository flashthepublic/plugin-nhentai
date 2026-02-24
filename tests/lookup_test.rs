use extism::*;
use rs_plugin_common_interfaces::{
    domain::rs_ids::RsIds,
    lookup::{
        RsLookupBook, RsLookupMetadataResult, RsLookupMetadataResults,
        RsLookupMetadataResultWrapper, RsLookupQuery, RsLookupSourceResult, RsLookupWrapper,
    },
};

fn build_plugin() -> Plugin {
    let wasm = Wasm::file("target/wasm32-unknown-unknown/release/rs_plugin_nh.wasm");
    let manifest = Manifest::new([wasm]).with_allowed_host("nhentai.net");
    Plugin::new(&manifest, [], true).expect("Failed to create plugin")
}

fn call_lookup_source(plugin: &mut Plugin, input: &RsLookupWrapper) -> RsLookupSourceResult {
    let input_str = serde_json::to_string(input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup", &input_str)
        .expect("lookup call failed");
    serde_json::from_slice(output).expect("Failed to parse lookup source result")
}

fn call_lookup(plugin: &mut Plugin, input: &RsLookupWrapper) -> RsLookupMetadataResults {
    let input_str = serde_json::to_string(input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup_metadata", &input_str)
        .expect("lookup_metadata call failed");
    serde_json::from_slice(output).expect("Failed to parse lookup output")
}

#[test]
fn test_lookup_empty_name_returns_404() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some(String::new()),
            ids: None,
            page_key: None,
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
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for 'cheating'"
    );

    let first = &results.results[0];
    let book = match &first.metadata {
        RsLookupMetadataResult::Book(book) => book,
        _ => panic!("Expected book metadata"),
    };
    assert!(
        !book.name.trim().is_empty(),
        "Expected a non-empty book name in the first result"
    );

    assert!(
        first
            .relations
            .as_ref()
            .and_then(|relations| relations.ext_images.as_ref())
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
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for direct id nhentai:629637"
    );

    let first = &results.results[0];
    let book = match &first.metadata {
        RsLookupMetadataResult::Book(book) => book,
        _ => panic!("Expected book metadata"),
    };

    assert_eq!(
        Some(book.id.as_str()),
        Some("nhentai:629637"),
        "Expected direct-id lookup to preserve nhentai id"
    );

    assert_eq!(
        book.params
            .as_ref()
            .and_then(|v| v.get("nhentaiId"))
            .and_then(|v| v.as_str()),
        Some("629637"),
        "Expected params.nhentaiId to match the requested id"
    );

    assert_eq!(
        book.params
            .as_ref()
            .and_then(|v| v.get("nhentaiUrl"))
            .and_then(|v| v.as_str()),
        Some("https://nhentai.net/g/629637/"),
        "Expected params.nhentaiUrl to point to the direct gallery URL"
    );

    assert!(
        book.params
            .as_ref()
            .and_then(|v| v.get("artists"))
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "Expected at least one artist extracted from the gallery page"
    );

    assert!(
        first
            .relations
            .as_ref()
            .and_then(|relations| relations.ext_images.as_ref())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "Expected at least one image in direct-id lookup result"
    );
    /*
    println!(
        "Direct ID lookup result: tags {:?}",
        first
            .relations
            .as_ref()
            .and_then(|relations| relations.tags_details.as_ref())
    );
    println!(
        "Direct ID lookup result: people {:?}",
        first
            .relations
            .as_ref()
            .and_then(|relations| relations.people_details.as_ref())
    );*/
}

#[test]
fn test_lookup_direct_id_624988_parodies_returned() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("nhentai:624988".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for direct id nhentai:624988"
    );

    let first = &results.results[0];
    let book = match &first.metadata {
        RsLookupMetadataResult::Book(book) => book,
        _ => panic!("Expected book metadata"),
    };

    assert_eq!(
        Some(book.id.as_str()),
        Some("nhentai:624988"),
        "Expected direct-id lookup to preserve nhentai id"
    );

    assert!(
        book.params
            .as_ref()
            .and_then(|v| v.get("parodies"))
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "Expected at least one parody extracted from the gallery page"
    );

    println!(
        "Direct ID lookup result: tags {:?}",
        first
            .relations
            .as_ref()
            .and_then(|relations| relations.series.as_ref())
    );
}

#[test]
fn test_lookup_571095_returns_group_download() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("test".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["nhentai:571095".to_string()].into()),
                ..Default::default()
            }),
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let input_str = serde_json::to_string(&input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup", &input_str)
        .expect("lookup call failed");
    let result: RsLookupSourceResult =
        serde_json::from_slice(output).expect("Failed to parse lookup output");

    match result {
        RsLookupSourceResult::GroupRequest(groups) => {
            assert!(!groups.is_empty(), "Expected at least one group");
            let group = &groups[0];
            assert!(group.group, "Expected group flag to be true");
            assert!(
                !group.requests.is_empty(),
                "Expected multiple image requests in group"
            );
            println!("Got {} images for nhentai:571095", group.requests.len());
            for req in &group.requests {
                println!("  {}", req.url);
            }
        }
        other => panic!("Expected GroupRequest, got {:?}", other),
    }
}

#[test]
fn test_lookup_metadata_falls_back_to_name_search_on_unknown_id() {
    let mut plugin = build_plugin();

    // nhentai:999999999 should not exist; the name "cheating" should produce search results.
    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["nhentai:999999999".to_string()].into()),
                ..Default::default()
            }),
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected search fallback to return results when direct ID is unknown"
    );
    let book = match &results.results[0].metadata {
        RsLookupMetadataResult::Book(book) => book,
        _ => panic!("Expected book metadata"),
    };
    assert!(
        !book.name.trim().is_empty(),
        "Expected a non-empty book name from fallback search"
    );
}

#[test]
fn test_lookup_returns_group_for_name_only_search() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let result = call_lookup_source(&mut plugin, &input);
    let RsLookupSourceResult::GroupRequest(groups) = result else {
        panic!("Expected GroupRequest for name-only search");
    };
    assert!(
        !groups.is_empty(),
        "Expected at least one group from name search"
    );
    assert!(
        groups.iter().all(|g| g.group),
        "Expected all results to have group flag set"
    );
    println!("Name-only search returned {} groups", groups.len());
}

#[test]
fn test_lookup_falls_back_to_name_search_on_unknown_id() {
    let mut plugin = build_plugin();

    // nhentai:999999999 should not exist; the name "cheating" should produce search results.
    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["nhentai:999999999".to_string()].into()),
                ..Default::default()
            }),
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let result = call_lookup_source(&mut plugin, &input);
    let RsLookupSourceResult::GroupRequest(groups) = result else {
        panic!("Expected GroupRequest from name fallback after unknown ID");
    };
    assert!(
        !groups.is_empty(),
        "Expected search fallback to produce groups"
    );
    println!(
        "Fallback search returned {} groups, first has {} requests",
        groups.len(),
        groups[0].requests.len()
    );
}

#[test]
fn test_lookup_metadata_search_returns_next_page_key() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for 'cheating'"
    );
    assert_eq!(
        results.next_page_key,
        Some("2".to_string()),
        "Expected next_page_key to be '2' for the first page of search results"
    );
    println!(
        "Search returned {} results, next_page_key: {:?}",
        results.results.len(),
        results.next_page_key
    );
}

#[test]
fn test_lookup_metadata_relation_id_artist_search() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("nhentai-artist:bai-asuka".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for artist relation ID 'nhentai-artist:bai-asuka'"
    );

    let first = &results.results[0];
    let book = match &first.metadata {
        RsLookupMetadataResult::Book(book) => book,
        _ => panic!("Expected book metadata"),
    };
    assert!(
        !book.name.trim().is_empty(),
        "Expected a non-empty book name from artist relation ID search"
    );
    println!(
        "Relation ID artist search returned {} results, first: {}",
        results.results.len(),
        book.name
    );
}

#[test]
fn test_lookup_metadata_page_2_returns_different_results() {
    let mut plugin = build_plugin();

    let page1_input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let page1 = call_lookup(&mut plugin, &page1_input);
    assert!(!page1.results.is_empty(), "Expected page 1 results");

    let page2_input = RsLookupWrapper {
        query: RsLookupQuery::Book(RsLookupBook {
            name: Some("cheating".to_string()),
            ids: None,
            page_key: Some("2".to_string()),
        }),
        credential: None,
        params: None,
    };

    let page2 = call_lookup(&mut plugin, &page2_input);
    assert!(!page2.results.is_empty(), "Expected page 2 results");

    // Extract first result IDs from each page to confirm they differ
    let page1_first_id = match &page1.results[0].metadata {
        RsLookupMetadataResult::Book(book) => book.id.clone(),
        _ => panic!("Expected book metadata on page 1"),
    };
    let page2_first_id = match &page2.results[0].metadata {
        RsLookupMetadataResult::Book(book) => book.id.clone(),
        _ => panic!("Expected book metadata on page 2"),
    };

    assert_ne!(
        page1_first_id, page2_first_id,
        "Expected page 1 and page 2 to return different results"
    );
    println!(
        "Page 1 first: {}, Page 2 first: {}, Page 2 next_page_key: {:?}",
        page1_first_id, page2_first_id, page2.next_page_key
    );
}
