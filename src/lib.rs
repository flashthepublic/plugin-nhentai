use extism_pdk::{http, log, plugin_fn, FnResult, HttpRequest, Json, LogLevel, WithReturnCode};
use std::collections::HashSet;

use rs_plugin_common_interfaces::{
    domain::external_images::ExternalImage,
    lookup::{RsLookupBook, RsLookupMetadataResultWrapper, RsLookupQuery, RsLookupWrapper},
    PluginInformation, PluginType,
};

mod convert;
mod nhentai;

use convert::{nhentai_gallery_to_images, nhentai_gallery_to_result};
use nhentai::{
    build_gallery_url, build_search_url, parse_gallery_html, parse_lookup_gallery_id,
    parse_search_html, NhentaiGallery,
};

enum LookupTarget<'a> {
    DirectGallery(String),
    Search(&'a str),
}

#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(PluginInformation {
        name: "nhentai_metadata".into(),
        capabilities: vec![PluginType::LookupMetadata],
        version: 5,
        interface_version: 1,
        repo: Some("https://github.com/flashthepublic/plugin-nhentai".to_string()),
        publisher: "neckaros".into(),
        description: "Look up books metadata from nhentai.net".into(),
        credential_kind: None,
        settings: vec![],
        ..Default::default()
    }))
}

fn build_http_request(url: String) -> HttpRequest {
    let mut request = HttpRequest {
        url,
        headers: Default::default(),
        method: Some("GET".into()),
    };

    request
        .headers
        .insert("Accept".to_string(), "text/html".to_string());
    request.headers.insert(
        "User-Agent".to_string(),
        "rs-plugin-nh/0.1 (+https://nhentai.net)".to_string(),
    );

    request
}

fn execute_search_request(search: &str) -> FnResult<Vec<NhentaiGallery>> {
    let url = build_search_url(search)
        .ok_or_else(|| WithReturnCode::new(extism_pdk::Error::msg("Not supported"), 404))?;

    let body = execute_html_request(url)?;
    Ok(parse_search_html(&body))
}

fn execute_gallery_request(gallery_id: &str) -> FnResult<Vec<NhentaiGallery>> {
    let body = execute_html_request(build_gallery_url(gallery_id))?;
    Ok(parse_gallery_html(&body, gallery_id).into_iter().collect())
}

fn execute_html_request(url: String) -> FnResult<String> {
    let request = build_http_request(url);
    let res = http::request::<Vec<u8>>(&request, None);

    match res {
        Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
            Ok(String::from_utf8_lossy(&res.body()).to_string())
        }
        Ok(res) => {
            log!(
                LogLevel::Error,
                "nhentai HTTP error {}: {}",
                res.status_code(),
                String::from_utf8_lossy(&res.body())
            );
            Err(WithReturnCode::new(
                extism_pdk::Error::msg(format!("HTTP error: {}", res.status_code())),
                res.status_code() as i32,
            ))
        }
        Err(e) => {
            log!(LogLevel::Error, "nhentai request failed: {}", e);
            Err(WithReturnCode(e, 500))
        }
    }
}

fn lookup_galleries(lookup: &RsLookupWrapper) -> FnResult<Vec<NhentaiGallery>> {
    let book = match &lookup.query {
        RsLookupQuery::Book(book) => book,
        _ => return Ok(vec![]),
    };

    match resolve_book_lookup_target(book) {
        Some(LookupTarget::DirectGallery(gallery_id)) => execute_gallery_request(&gallery_id),
        Some(LookupTarget::Search(search)) => execute_search_request(search),
        _ => Err(WithReturnCode::new(
            extism_pdk::Error::msg("Not supported"),
            404,
        )),
    }
}

fn resolve_book_lookup_target(book: &RsLookupBook) -> Option<LookupTarget<'_>> {
    if let Some(id) = book.name.as_deref().and_then(parse_lookup_gallery_id) {
        return Some(LookupTarget::DirectGallery(id));
    }

    if let Some(ids) = book.ids.as_ref() {
        if let Some(id) = ids.redseat.as_deref().and_then(parse_lookup_gallery_id) {
            return Some(LookupTarget::DirectGallery(id));
        }

        if let Some(id) = ids.slug.as_deref().and_then(parse_lookup_gallery_id) {
            return Some(LookupTarget::DirectGallery(id));
        }

        if let Some(id) = ids.other_ids.as_ref().and_then(|other_ids| {
            other_ids
                .as_slice()
                .iter()
                .find_map(|value| parse_lookup_gallery_id(value))
        }) {
            return Some(LookupTarget::DirectGallery(id));
        }
    }

    book.name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(LookupTarget::Search)
}

#[plugin_fn]
pub fn lookup_metadata(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<Vec<RsLookupMetadataResultWrapper>>> {
    let galleries = lookup_galleries(&lookup)?;

    let results = galleries
        .into_iter()
        .map(nhentai_gallery_to_result)
        .collect();

    Ok(Json(results))
}

#[plugin_fn]
pub fn lookup_metadata_images(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<Vec<ExternalImage>>> {
    let galleries = lookup_galleries(&lookup)?;

    let images: Vec<ExternalImage> = galleries
        .iter()
        .flat_map(nhentai_gallery_to_images)
        .collect();

    Ok(Json(deduplicate_images(images)))
}

fn deduplicate_images(images: Vec<ExternalImage>) -> Vec<ExternalImage> {
    let mut seen_urls = HashSet::new();
    let mut deduped = Vec::new();

    for image in images {
        if seen_urls.insert(image.url.url.clone()) {
            deduped.push(image);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_plugin_common_interfaces::domain::rs_ids::RsIds;

    #[test]
    fn lookup_non_book_query_returns_empty() {
        let lookup = RsLookupWrapper {
            query: RsLookupQuery::Movie(Default::default()),
            credential: None,
            params: None,
        };

        let result = lookup_galleries(&lookup).expect("lookup should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn lookup_empty_book_name_returns_404() {
        let lookup = RsLookupWrapper {
            query: RsLookupQuery::Book(RsLookupBook {
                name: Some(String::new()),
                ids: None,
            }),
            credential: None,
            params: None,
        };

        let err = lookup_galleries(&lookup).expect_err("expected 404");
        assert_eq!(err.1, 404);
    }

    #[test]
    fn resolve_target_prefers_direct_name_id() {
        let book = RsLookupBook {
            name: Some("nhentai:12345".to_string()),
            ids: None,
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::DirectGallery(id)) => assert_eq!(id, "12345"),
            _ => panic!("Expected direct gallery target"),
        }
    }

    #[test]
    fn resolve_target_reads_ids_other_ids() {
        let book = RsLookupBook {
            name: Some("ignored text".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["nhentai:67890".to_string()].into()),
                ..Default::default()
            }),
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::DirectGallery(id)) => assert_eq!(id, "67890"),
            _ => panic!("Expected direct gallery target from ids"),
        }
    }

    #[test]
    fn deduplicate_images_by_url() {
        let images = vec![
            ExternalImage {
                url: rs_plugin_common_interfaces::RsRequest {
                    url: "https://a.com/1.jpg".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ExternalImage {
                url: rs_plugin_common_interfaces::RsRequest {
                    url: "https://a.com/1.jpg".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let deduped = deduplicate_images(images);
        assert_eq!(deduped.len(), 1);
    }
}
