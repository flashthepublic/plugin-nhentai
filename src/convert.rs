use rs_plugin_common_interfaces::{
    domain::{
        book::Book,
        external_images::{ExternalImage, ImageType},
        media::FileEpisode,
        person::Person,
        tag::Tag,
        Relations,
    },
    lookup::{RsLookupMetadataResult, RsLookupMetadataResultWrapper},
    RsRequest,
};
use serde_json::json;

use crate::nhentai::{NhentaiGallery, NhentaiRelation};

pub fn nhentai_gallery_to_result(item: NhentaiGallery) -> RsLookupMetadataResultWrapper {
    let images = nhentai_gallery_to_images(&item);
    let language_code = default_language_code(&item.languages);
    let people_details = build_people_details(&item.people_details);
    let tag_details = build_tag_details(&item.tag_details);
    let series = build_series(&item.parody_details);

    let id = item
        .id
        .as_ref()
        .map(|gallery_id| format!("nhentai:{gallery_id}"))
        .unwrap_or_else(|| fallback_local_id(&item.title));

    let params = json!({
        "nhentaiUrl": item.gallery_url,
        "nhentaiId": item.id,
        "tags": item.tags,
        "artists": item.artists,
        "groups": item.groups,
        "parodies": item.parodies,
        "characters": item.characters,
        "languages": item.languages,
        "categories": item.categories,
    });

    let book = Book {
        id,
        name: item.title,
        kind: Some("book".to_string()),
        lang: language_code,
        pages: item.pages,
        params: Some(params),
        ..Default::default()
    };

    RsLookupMetadataResultWrapper {
        metadata: RsLookupMetadataResult::Book(book),
        relations: Some(Relations {
            ext_images: if images.is_empty() {
                None
            } else {
                Some(images)
            },
            people_details: if people_details.is_empty() {
                None
            } else {
                Some(people_details)
            },
            tags_details: if tag_details.is_empty() {
                None
            } else {
                Some(tag_details)
            },
            series: if series.is_empty() {
                None
            } else {
                Some(series)
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn nhentai_gallery_to_images(item: &NhentaiGallery) -> Vec<ExternalImage> {
    let urls = if item.images.is_empty() {
        vec![item.cover_url.clone()]
    } else {
        item.images.clone()
    };

    urls.into_iter()
        .filter(|url| !url.trim().is_empty())
        .enumerate()
        .map(|(idx, url)| ExternalImage {
            kind: Some(if idx == 0 {
                ImageType::Poster
            } else {
                ImageType::Still
            }),
            url: RsRequest {
                url,
                ..Default::default()
            },
            ..Default::default()
        })
        .collect()
}

fn default_language_code(languages: &[String]) -> Option<String> {
    for language in languages {
        match language.to_ascii_lowercase().as_str() {
            "english" => return Some("en".to_string()),
            "japanese" => return Some("ja".to_string()),
            "chinese" => return Some("zh".to_string()),
            "korean" => return Some("ko".to_string()),
            _ => {}
        }
    }

    if languages.is_empty() {
        Some("en".to_string())
    } else {
        None
    }
}

fn build_people_details(values: &[NhentaiRelation]) -> Vec<Person> {
    values
        .iter()
        .filter(|value| !value.id.trim().is_empty() && !value.name.trim().is_empty())
        .map(|value| Person {
            id: value.id.clone(),
            name: value.name.clone(),
            generated: true,
            ..Default::default()
        })
        .collect()
}

fn build_tag_details(values: &[NhentaiRelation]) -> Vec<Tag> {
    values
        .iter()
        .filter(|value| !value.id.trim().is_empty() && !value.name.trim().is_empty())
        .map(|value| Tag {
            id: value.id.clone(),
            name: value.name.clone(),
            parent: None,
            kind: None,
            alt: None,
            thumb: None,
            params: None,
            modified: 0,
            added: 0,
            generated: true,
            otherids: Some(vec![value.id.clone()].into()),
            path: "/".to_string(),
        })
        .collect()
}

fn build_series(values: &[NhentaiRelation]) -> Vec<FileEpisode> {
    values
        .iter()
        .filter(|value| {
            !value.id.trim().is_empty()
                && !value.name.trim().is_empty()
                && value.name.to_ascii_lowercase() != "original"
        })
        .map(|value| FileEpisode {
            id: value.id.clone(),
            season: None,
            episode: None,
            episode_to: None,
        })
        .collect()
}

fn fallback_local_id(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "nhentai-title".to_string()
    } else {
        format!("nhentai-title-{slug}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_gallery_to_book_result() {
        let result = nhentai_gallery_to_result(NhentaiGallery {
            id: Some("12345".to_string()),
            title: "Soft Sample".to_string(),
            cover_url: "https://t3.nhentai.net/galleries/111/thumb.jpg".to_string(),
            gallery_url: "https://nhentai.net/g/12345/".to_string(),
            ..Default::default()
        });

        if let RsLookupMetadataResult::Book(book) = result.metadata {
            assert_eq!(book.id, "nhentai:12345");
            assert_eq!(book.name, "Soft Sample");
            assert_eq!(book.lang, Some("en".to_string()));
        } else {
            panic!("Expected book metadata");
        }

        let images = result
            .relations
            .as_ref()
            .and_then(|relations| relations.ext_images.as_ref())
            .expect("expected ext_images");
        assert_eq!(images.len(), 1);
        assert_eq!(
            images[0].url.url,
            "https://t3.nhentai.net/galleries/111/thumb.jpg"
        );
    }

    #[test]
    fn maps_all_gallery_images_when_available() {
        let gallery = NhentaiGallery {
            title: "Many Pages".to_string(),
            cover_url: "https://t.nhentai.net/galleries/1/cover.jpg".to_string(),
            gallery_url: "https://nhentai.net/g/1/".to_string(),
            images: vec![
                "https://i.nhentai.net/galleries/1/1.jpg".to_string(),
                "https://i.nhentai.net/galleries/1/2.jpg".to_string(),
            ],
            ..Default::default()
        };

        let images = nhentai_gallery_to_images(&gallery);
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].kind, Some(ImageType::Poster));
        assert_eq!(images[1].kind, Some(ImageType::Still));
    }

    #[test]
    fn maps_people_and_tags_relations() {
        let result = nhentai_gallery_to_result(NhentaiGallery {
            id: Some("12345".to_string()),
            title: "Soft Sample".to_string(),
            cover_url: "https://t3.nhentai.net/galleries/111/thumb.jpg".to_string(),
            gallery_url: "https://nhentai.net/g/12345/".to_string(),
            people_details: vec![NhentaiRelation {
                id: "nhentai-artist:bai-asuka".to_string(),
                name: "bai asuka".to_string(),
            }],
            tag_details: vec![NhentaiRelation {
                id: "nhentai-tags:full-color".to_string(),
                name: "full color".to_string(),
            }],
            ..Default::default()
        });

        let relations = result.relations.expect("expected relations");
        let people = relations.people_details.expect("expected people_details");
        let tags = relations.tags_details.expect("expected tags_details");

        assert_eq!(people[0].id, "nhentai-artist:bai-asuka");
        assert_eq!(people[0].name, "bai asuka");
        assert_eq!(tags[0].id, "nhentai-tags:full-color");
        assert_eq!(tags[0].name, "full color");
        assert!(relations.people.is_none());
        assert!(relations.tags.is_none());
    }
}
