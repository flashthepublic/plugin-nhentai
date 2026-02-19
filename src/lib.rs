use extism_pdk::{http, log, plugin_fn, FnResult, HttpRequest, Json, LogLevel, WithReturnCode};
use std::collections::HashSet;

use rs_plugin_common_interfaces::{
    domain::external_images::ExternalImage,
    lookup::{RsLookupMetadataResultWithImages, RsLookupQuery, RsLookupWrapper},
    PluginInformation, PluginType,
};

mod convert;
mod nhentai;

use convert::{nhentai_gallery_to_images, nhentai_gallery_to_result};
use nhentai::{build_search_url, parse_search_html, NhentaiGallery};

#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(PluginInformation {
        name: "nhentai_metadata".into(),
        capabilities: vec![PluginType::LookupMetadata],
        version: 1,
        interface_version: 1,
        repo: None,
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

    let request = build_http_request(url);
    let res = http::request::<Vec<u8>>(&request, None);

    match res {
        Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
            let body = String::from_utf8_lossy(&res.body()).to_string();
            Ok(parse_search_html(&body))
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
    let search = match &lookup.query {
        RsLookupQuery::Book(book) => book.name.as_deref(),
        _ => return Ok(vec![]),
    };

    match search {
        Some(s) if !s.trim().is_empty() => execute_search_request(s),
        _ => Err(WithReturnCode::new(
            extism_pdk::Error::msg("Not supported"),
            404,
        )),
    }
}

#[plugin_fn]
pub fn lookup_metadata(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<Vec<RsLookupMetadataResultWithImages>>> {
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
    use rs_plugin_common_interfaces::lookup::RsLookupBook;

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
