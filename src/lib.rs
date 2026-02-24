use extism_pdk::{http, log, plugin_fn, FnResult, HttpRequest, Json, LogLevel, WithReturnCode};
use std::collections::HashSet;

use rs_plugin_common_interfaces::{
    domain::external_images::ExternalImage,
    domain::media::{FileEpisode, MediaForUpdate, MediaItemReference},
    lookup::{
        RsLookupBook, RsLookupMetadataResults, RsLookupQuery, RsLookupSourceResult,
        RsLookupWrapper,
    },
    request::{RsGroupDownload, RsRequest},
    CustomParam, CustomParamTypes, PluginInformation, PluginType,
};

mod convert;
mod nhentai;

use convert::{nhentai_gallery_to_images, nhentai_gallery_to_result};
use nhentai::{
    build_gallery_url, build_search_url, parse_gallery_html, parse_lookup_gallery_id,
    parse_relation_search_term, parse_search_html, parse_search_next_page, NhentaiGallery,
};

enum LookupTarget {
    DirectGallery(String),
    Search(String),
}

#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(PluginInformation {
        name: "nhentai_metadata".into(),
        capabilities: vec![PluginType::LookupMetadata, PluginType::Lookup],
        version: 11,
        interface_version: 1,
        repo: Some("https://github.com/flashthepublic/plugin-nhentai".to_string()),
        publisher: "neckaros".into(),
        description: "Look up books metadata from nhentai.net".into(),
        credential_kind: None,
        settings: vec![CustomParam {
            name: "custom_search_params".into(),
            param: CustomParamTypes::Text(None),
            description: Some("Custom parameters appended to every search query".into()),
            required: false,
        }],
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

fn execute_search_request(
    search: &str,
    page: Option<u32>,
    custom_search_params: Option<&str>,
) -> FnResult<(Vec<NhentaiGallery>, Option<String>)> {
    let url = build_search_url(search, page, custom_search_params)
        .ok_or_else(|| WithReturnCode::new(extism_pdk::Error::msg("Not supported"), 404))?;

    let body = execute_html_request(url)?;
    let galleries = parse_search_html(&body);
    let current_page = page.unwrap_or(1);
    let next_page_key = if galleries.is_empty() {
        None
    } else {
        parse_search_next_page(&body, current_page).map(|p| p.to_string())
    };
    Ok((galleries, next_page_key))
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

fn lookup_galleries(
    lookup: &RsLookupWrapper,
) -> FnResult<(Vec<NhentaiGallery>, Option<String>)> {
    let book = match &lookup.query {
        RsLookupQuery::Book(book) => book,
        _ => return Ok((vec![], None)),
    };

    let custom_search_params = lookup
        .params
        .as_ref()
        .and_then(|p| p.get("custom_search_params"))
        .map(|s| s.as_str());

    let page = book
        .page_key
        .as_deref()
        .and_then(|k| k.parse::<u32>().ok());

    match resolve_book_lookup_target(book) {
        Some(LookupTarget::DirectGallery(gallery_id)) => {
            let galleries = execute_gallery_request(&gallery_id).unwrap_or_default();
            if !galleries.is_empty() {
                return Ok((galleries, None));
            }
            // Gallery lookup returned nothing; fall back to name search if available.
            match book
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                Some(name) => execute_search_request(name, page, custom_search_params),
                None => Ok((vec![], None)),
            }
        }
        Some(LookupTarget::Search(search)) => execute_search_request(&search, page, custom_search_params),
        _ => Err(WithReturnCode::new(
            extism_pdk::Error::msg("Not supported"),
            404,
        )),
    }
}

fn resolve_book_lookup_target(book: &RsLookupBook) -> Option<LookupTarget> {
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

    // Check for relation IDs (e.g. "nhentai-group:maiju") and convert to search terms.
    if let Some(term) = book.name.as_deref().and_then(parse_relation_search_term) {
        return Some(LookupTarget::Search(term));
    }

    if let Some(ids) = book.ids.as_ref() {
        if let Some(term) = ids.redseat.as_deref().and_then(parse_relation_search_term) {
            return Some(LookupTarget::Search(term));
        }

        if let Some(term) = ids.slug.as_deref().and_then(parse_relation_search_term) {
            return Some(LookupTarget::Search(term));
        }

        if let Some(term) = ids.other_ids.as_ref().and_then(|other_ids| {
            other_ids
                .as_slice()
                .iter()
                .find_map(|value| parse_relation_search_term(value))
        }) {
            return Some(LookupTarget::Search(term));
        }
    }

    book.name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| LookupTarget::Search(value.to_string()))
}

#[plugin_fn]
pub fn lookup_metadata(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<RsLookupMetadataResults>> {
    let (galleries, next_page_key) = lookup_galleries(&lookup)?;

    let results = galleries
        .into_iter()
        .map(nhentai_gallery_to_result)
        .collect();

    Ok(Json(RsLookupMetadataResults {
        results,
        next_page_key,
    }))
}

#[plugin_fn]
pub fn lookup_metadata_images(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<Vec<ExternalImage>>> {
    let (galleries, _) = lookup_galleries(&lookup)?;

    let images: Vec<ExternalImage> = galleries
        .iter()
        .flat_map(nhentai_gallery_to_images)
        .collect();

    Ok(Json(deduplicate_images(images)))
}

#[plugin_fn]
pub fn lookup(Json(lookup): Json<RsLookupWrapper>) -> FnResult<Json<RsLookupSourceResult>> {
    let book = match &lookup.query {
        RsLookupQuery::Book(book) => book,
        _ => return Ok(Json(RsLookupSourceResult::NotApplicable)),
    };

    let custom_search_params = lookup
        .params
        .as_ref()
        .and_then(|p| p.get("custom_search_params"))
        .map(|s| s.as_str());

    match resolve_book_lookup_target(book) {
        Some(LookupTarget::DirectGallery(gallery_id)) => {
            let galleries = execute_gallery_request(&gallery_id).unwrap_or_default();
            if !galleries.is_empty() {
                return Ok(Json(galleries_to_group_result(galleries)));
            }
            // Fall back to name search if the gallery returned nothing.
            match book
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                Some(name) => {
                    let (galleries, _) = execute_search_request(name, None, custom_search_params)?;
                    Ok(Json(galleries_to_group_result(galleries)))
                }
                None => Ok(Json(RsLookupSourceResult::NotFound)),
            }
        }
        Some(LookupTarget::Search(search)) => {
            let (galleries, _) = execute_search_request(&search, None, custom_search_params)?;
            Ok(Json(galleries_to_group_result(galleries)))
        }
        _ => Ok(Json(RsLookupSourceResult::NotApplicable)),
    }
}

fn gallery_to_group_download(gallery: NhentaiGallery) -> RsGroupDownload {
    let requests: Vec<RsRequest> = gallery
        .images
        .iter()
        .map(|url| RsRequest {
            url: url.clone(),
            permanent: true,
            mime: url.split('.').last().map(|ext| format!("image/{}", ext)),
            instant: Some(true),
            ..Default::default()
        })
        .collect();

    let infos = gallery_to_infos(&gallery);

    RsGroupDownload {
        group: true,
        group_thumbnail_url: if gallery.cover_url.is_empty() {
            None
        } else {
            Some(gallery.cover_url.clone())
        },
        requests,
        infos,
        ..Default::default()
    }
}

fn gallery_to_infos(gallery: &NhentaiGallery) -> Option<MediaForUpdate> {
    let add_people = relation_details_to_media_refs(&gallery.people_details);
    let add_tags = relation_details_to_media_refs(&gallery.tag_details);
    let add_series = relation_details_to_series_refs(&gallery.parody_details);

    let people_lookup = relation_details_to_lookup_names(&gallery.people_details);
    let tags_lookup = relation_details_to_lookup_names(&gallery.tag_details);
    let series_lookup = relation_details_to_series_lookup_names(&gallery.parody_details);

    if add_people.is_empty()
        && add_tags.is_empty()
        && add_series.is_empty()
        && people_lookup.is_empty()
        && tags_lookup.is_empty()
        && series_lookup.is_empty()
    {
        return None;
    }

    Some(MediaForUpdate {
        add_people: if add_people.is_empty() {
            None
        } else {
            Some(add_people)
        },
        add_tags: if add_tags.is_empty() {
            None
        } else {
            Some(add_tags)
        },
        add_series: if add_series.is_empty() {
            None
        } else {
            Some(add_series)
        },
        people_lookup: if people_lookup.is_empty() {
            None
        } else {
            Some(people_lookup)
        },
        tags_lookup: if tags_lookup.is_empty() {
            None
        } else {
            Some(tags_lookup)
        },
        series_lookup: if series_lookup.is_empty() {
            None
        } else {
            Some(series_lookup)
        },
        ..Default::default()
    })
}

fn relation_details_to_media_refs(values: &[nhentai::NhentaiRelation]) -> Vec<MediaItemReference> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter_map(|value| {
            let id = value.id.trim();
            let name = value.name.trim();
            if id.is_empty() || name.is_empty() || !seen.insert(id.to_string()) {
                None
            } else {
                Some(MediaItemReference {
                    id: id.to_string(),
                    conf: None,
                })
            }
        })
        .collect()
}

fn relation_details_to_series_refs(values: &[nhentai::NhentaiRelation]) -> Vec<FileEpisode> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter_map(|value| {
            let id = value.id.trim();
            let name = value.name.trim();
            if id.is_empty()
                || name.is_empty()
                || name.eq_ignore_ascii_case("original")
                || !seen.insert(id.to_string())
            {
                None
            } else {
                Some(FileEpisode {
                    id: id.to_string(),
                    season: None,
                    episode: None,
                    episode_to: None,
                })
            }
        })
        .collect()
}

fn relation_details_to_lookup_names(values: &[nhentai::NhentaiRelation]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter_map(|value| {
            let name = value.name.trim();
            if name.is_empty() || !seen.insert(name.to_string()) {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn relation_details_to_series_lookup_names(values: &[nhentai::NhentaiRelation]) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .filter_map(|value| {
            let name = value.name.trim();
            if name.is_empty()
                || name.eq_ignore_ascii_case("original")
                || !seen.insert(name.to_string())
            {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn galleries_to_group_result(galleries: Vec<NhentaiGallery>) -> RsLookupSourceResult {
    if galleries.is_empty() {
        return RsLookupSourceResult::NotFound;
    }
    let group_downloads = galleries
        .into_iter()
        .map(gallery_to_group_download)
        .collect();
    RsLookupSourceResult::GroupRequest(group_downloads)
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

        let (galleries, _) = lookup_galleries(&lookup).expect("lookup should succeed");
        assert!(galleries.is_empty());
    }

    #[test]
    fn lookup_empty_book_name_returns_404() {
        let lookup = RsLookupWrapper {
            query: RsLookupQuery::Book(RsLookupBook {
                name: Some(String::new()),
                ids: None,
                page_key: None,
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
            page_key: None,
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
            page_key: None,
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::DirectGallery(id)) => assert_eq!(id, "67890"),
            _ => panic!("Expected direct gallery target from ids"),
        }
    }

    #[test]
    fn galleries_to_group_result_empty_returns_not_found() {
        let result = galleries_to_group_result(vec![]);
        assert!(matches!(result, RsLookupSourceResult::NotFound));
    }

    #[test]
    fn galleries_to_group_result_maps_each_gallery() {
        let galleries = vec![
            NhentaiGallery {
                id: Some("1".to_string()),
                title: "Gallery One".to_string(),
                cover_url: "https://t.nhentai.net/galleries/1/cover.jpg".to_string(),
                images: vec![
                    "https://i.nhentai.net/galleries/1/1.jpg".to_string(),
                    "https://i.nhentai.net/galleries/1/2.jpg".to_string(),
                ],
                ..Default::default()
            },
            NhentaiGallery {
                id: Some("2".to_string()),
                title: "Gallery Two".to_string(),
                cover_url: "https://t.nhentai.net/galleries/2/cover.jpg".to_string(),
                images: vec!["https://i.nhentai.net/galleries/2/1.png".to_string()],
                ..Default::default()
            },
        ];

        let result = galleries_to_group_result(galleries);
        let RsLookupSourceResult::GroupRequest(downloads) = result else {
            panic!("Expected GroupRequest");
        };
        assert_eq!(downloads.len(), 2);
        assert_eq!(downloads[0].requests.len(), 2);
        assert_eq!(
            downloads[0].group_thumbnail_url,
            Some("https://t.nhentai.net/galleries/1/cover.jpg".to_string())
        );
        assert_eq!(downloads[1].requests.len(), 1);
        assert_eq!(downloads[1].requests[0].mime, Some("image/png".to_string()));
    }

    #[test]
    fn gallery_to_group_download_sets_mime_from_extension() {
        let gallery = NhentaiGallery {
            cover_url: "https://t.nhentai.net/galleries/5/cover.jpg".to_string(),
            images: vec![
                "https://i.nhentai.net/galleries/5/1.jpg".to_string(),
                "https://i.nhentai.net/galleries/5/2.webp".to_string(),
            ],
            ..Default::default()
        };

        let download = gallery_to_group_download(gallery);
        assert_eq!(download.requests[0].mime, Some("image/jpg".to_string()));
        assert_eq!(download.requests[1].mime, Some("image/webp".to_string()));
        assert!(download.requests[0].permanent);
        assert_eq!(download.requests[0].instant, Some(true));
    }

    #[test]
    fn gallery_to_group_download_empty_cover_sets_no_thumbnail() {
        let gallery = NhentaiGallery {
            cover_url: String::new(),
            images: vec!["https://i.nhentai.net/galleries/6/1.jpg".to_string()],
            ..Default::default()
        };

        let download = gallery_to_group_download(gallery);
        assert!(download.group_thumbnail_url.is_none());
    }

    #[test]
    fn gallery_to_group_download_sets_infos_from_relation_details() {
        let gallery = NhentaiGallery {
            images: vec!["https://i.nhentai.net/galleries/7/1.jpg".to_string()],
            people_details: vec![nhentai::NhentaiRelation {
                id: "nhentai-artist:bai-asuka".to_string(),
                name: "bai asuka".to_string(),
            }],
            tag_details: vec![nhentai::NhentaiRelation {
                id: "nhentai-tags:full-color".to_string(),
                name: "full color".to_string(),
            }],
            parody_details: vec![
                nhentai::NhentaiRelation {
                    id: "nhentai-parody:naruto".to_string(),
                    name: "naruto".to_string(),
                },
                nhentai::NhentaiRelation {
                    id: "nhentai-parody:original".to_string(),
                    name: "original".to_string(),
                },
            ],
            ..Default::default()
        };

        let download = gallery_to_group_download(gallery);
        let infos = download.infos.expect("expected infos to be set");
        assert_eq!(
            infos.add_people.expect("expected add_people")[0].id,
            "nhentai-artist:bai-asuka"
        );
        assert_eq!(
            infos.add_tags.expect("expected add_tags")[0].id,
            "nhentai-tags:full-color"
        );
        assert_eq!(
            infos.add_series.expect("expected add_series")[0].id,
            "nhentai-parody:naruto"
        );
        assert_eq!(
            infos.people_lookup.expect("expected people_lookup")[0],
            "bai asuka"
        );
        assert_eq!(
            infos.tags_lookup.expect("expected tags_lookup")[0],
            "full color"
        );
        assert_eq!(
            infos.series_lookup.expect("expected series_lookup")[0],
            "naruto"
        );
    }

    #[test]
    fn resolve_target_relation_id_in_name() {
        let book = RsLookupBook {
            name: Some("nhentai-group:maiju".to_string()),
            ids: None,
            page_key: None,
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::Search(term)) => assert_eq!(term, "group:maiju"),
            _ => panic!("Expected Search target for relation ID in name"),
        }
    }

    #[test]
    fn resolve_target_relation_id_in_other_ids() {
        let book = RsLookupBook {
            name: Some("some book name".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["nhentai-artist:sasaki-musashi".to_string()].into()),
                ..Default::default()
            }),
            page_key: None,
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::Search(term)) => assert_eq!(term, "artist:sasaki-musashi"),
            _ => panic!("Expected Search target for relation ID in other_ids"),
        }
    }

    #[test]
    fn resolve_target_gallery_id_preferred_over_relation() {
        let book = RsLookupBook {
            name: Some("nhentai:12345".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["nhentai-artist:bai-asuka".to_string()].into()),
                ..Default::default()
            }),
            page_key: None,
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::DirectGallery(id)) => assert_eq!(id, "12345"),
            _ => panic!("Expected DirectGallery to win over relation ID"),
        }
    }

    #[test]
    fn resolve_target_relation_id_in_name_tags_maps_to_tag() {
        let book = RsLookupBook {
            name: Some("nhentai-tags:full-color".to_string()),
            ids: None,
            page_key: None,
        };

        let target = resolve_book_lookup_target(&book);
        match target {
            Some(LookupTarget::Search(term)) => assert_eq!(term, "tag:full-color"),
            _ => panic!("Expected Search target with tag: prefix"),
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
