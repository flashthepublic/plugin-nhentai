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

    let id = item
        .id
        .as_ref()
        .map(|gallery_id| format!("nhentai:{gallery_id}"))
        .unwrap_or_else(|| fallback_local_id(&item.title));

    let params = json!({
        "nhentaiUrl": item.gallery_url,
        "nhentaiId": item.id,
    });

    let book = Book {
        id,
        name: item.title,
        kind: Some("book".to_string()),
        lang: Some("en".to_string()),
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
    vec![ExternalImage {
        kind: Some(ImageType::Poster),
        url: RsRequest {
            url: item.cover_url.clone(),
            ..Default::default()
        },
        ..Default::default()
    }]
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
}
