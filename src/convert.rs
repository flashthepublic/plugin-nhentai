use rs_plugin_common_interfaces::{
    domain::{
        book::Book,
        external_images::{ExternalImage, ImageType},
    },
    lookup::{RsLookupMetadataResult, RsLookupMetadataResultWithImages},
    RsRequest,
};
use serde_json::json;

use crate::nhentai::NhentaiGallery;

pub fn nhentai_gallery_to_result(item: NhentaiGallery) -> RsLookupMetadataResultWithImages {
    let images = nhentai_gallery_to_images(&item);
    let language_code = default_language_code(&item.languages);

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

    RsLookupMetadataResultWithImages {
        metadata: RsLookupMetadataResult::Book(book),
        images,
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

        assert_eq!(result.images.len(), 1);
        assert_eq!(
            result.images[0].url.url,
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
}
