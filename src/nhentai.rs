use regex::Regex;
use scraper::{ElementRef, Html, Selector};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NhentaiRelation {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NhentaiGallery {
    pub id: Option<String>,
    pub title: String,
    pub cover_url: String,
    pub gallery_url: String,
    pub images: Vec<String>,
    pub tags: Vec<String>,
    pub artists: Vec<String>,
    pub groups: Vec<String>,
    pub parodies: Vec<String>,
    pub characters: Vec<String>,
    pub languages: Vec<String>,
    pub categories: Vec<String>,
    pub pages: Option<u32>,
    pub people_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub people_details: Vec<NhentaiRelation>,
    pub tag_details: Vec<NhentaiRelation>,
    pub parody_details: Vec<NhentaiRelation>,
}

pub fn build_search_url(search: &str, page: Option<u32>) -> Option<String> {
    let trimmed = search.trim();
    if trimmed.is_empty() {
        return None;
    }

    let query = format!("language:english {trimmed}");
    let mut url = format!(
        "https://nhentai.net/search/?q={}",
        encode_query_component(&query)
    );
    if let Some(p) = page {
        if p > 1 {
            url.push_str(&format!("&page={p}"));
        }
    }
    Some(url)
}

pub fn parse_search_next_page(html: &str, current_page: u32) -> Option<u32> {
    let document = Html::parse_document(html);
    let next_selector = Selector::parse("a.next").ok()?;
    if document.select(&next_selector).next().is_some() {
        Some(current_page + 1)
    } else {
        None
    }
}

pub fn build_gallery_url(gallery_id: &str) -> String {
    format!("https://nhentai.net/g/{gallery_id}/")
}

pub fn parse_lookup_gallery_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(id) = lower.strip_prefix("nhentai:") {
        if is_valid_gallery_id(id) {
            return Some(id.to_string());
        }
    }

    extract_gallery_id(trimmed)
}

pub fn parse_search_html(html: &str) -> Vec<NhentaiGallery> {
    let document = Html::parse_document(html);
    let gallery_selector = Selector::parse(".gallery").expect("valid .gallery selector");
    let caption_selector = Selector::parse(".caption").expect("valid .caption selector");
    let cover_selector = Selector::parse("a.cover").expect("valid a.cover selector");
    let image_selector = Selector::parse("img").expect("valid img selector");

    let mut items = Vec::new();

    for gallery in document.select(&gallery_selector) {
        let title = gallery
            .select(&caption_selector)
            .next()
            .map(|el| normalize_text(&el.text().collect::<String>()))
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        let anchor = gallery.select(&cover_selector).next();
        let href = anchor
            .as_ref()
            .and_then(|el| el.value().attr("href"))
            .unwrap_or_default();
        let gallery_url = normalize_url(href);
        if gallery_url.is_empty() {
            continue;
        }

        let image = anchor
            .as_ref()
            .and_then(|el| el.select(&image_selector).next())
            .or_else(|| gallery.select(&image_selector).next());

        let cover_url = image
            .as_ref()
            .and_then(|el| {
                el.value()
                    .attr("data-src")
                    .or_else(|| el.value().attr("src"))
            })
            .map(normalize_url)
            .unwrap_or_default();

        if cover_url.is_empty() {
            continue;
        }

        items.push(NhentaiGallery {
            id: extract_gallery_id(href),
            title,
            cover_url: cover_url.clone(),
            gallery_url,
            images: vec![cover_url],
            ..Default::default()
        });
    }

    items
}

pub fn parse_gallery_html(html: &str, gallery_id: &str) -> Option<NhentaiGallery> {
    let document = Html::parse_document(html);

    let title = parse_gallery_title(&document);
    let cover_url = parse_gallery_cover_url(&document);
    let tag_buckets = parse_tag_buckets(&document);

    let mut images = parse_script_image_urls(&document).unwrap_or_default();
    if images.is_empty() {
        images = parse_thumbnail_image_urls(&document);
    }

    if images.is_empty() && !cover_url.is_empty() {
        images.push(cover_url.clone());
    }

    let images = deduplicate_strings(images);
    let resolved_cover = if !cover_url.is_empty() {
        cover_url
    } else {
        images.first().cloned().unwrap_or_default()
    };

    let title = if title.is_empty() {
        format!("nhentai {gallery_id}")
    } else {
        title
    };

    let image_pages = u32::try_from(images.len()).ok().filter(|count| *count > 0);
    let pages = tag_buckets.pages.or(image_pages);

    Some(NhentaiGallery {
        id: Some(gallery_id.to_string()),
        title,
        cover_url: resolved_cover,
        gallery_url: build_gallery_url(gallery_id),
        images,
        tags: tag_buckets.tags,
        artists: tag_buckets.artists,
        groups: tag_buckets.groups,
        parodies: tag_buckets.parodies,
        characters: tag_buckets.characters,
        languages: tag_buckets.languages,
        categories: tag_buckets.categories,
        pages,
        people_ids: tag_buckets.people_ids,
        tag_ids: tag_buckets.tag_ids,
        people_details: tag_buckets.people_details,
        tag_details: tag_buckets.tag_details,
        parody_details: tag_buckets.parody_details,
    })
}

pub fn extract_gallery_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);

    let path = without_scheme
        .strip_prefix("nhentai.net/")
        .or_else(|| without_scheme.strip_prefix("www.nhentai.net/"))
        .unwrap_or(without_scheme)
        .trim_matches('/');

    let mut parts = path.split('/');

    let section = parts.next()?;
    let id = parts.next()?;

    if section == "g" && is_valid_gallery_id(id) {
        Some(id.to_string())
    } else {
        None
    }
}

#[derive(Default)]
struct TagBuckets {
    tags: Vec<String>,
    artists: Vec<String>,
    groups: Vec<String>,
    parodies: Vec<String>,
    characters: Vec<String>,
    languages: Vec<String>,
    categories: Vec<String>,
    pages: Option<u32>,
    people_ids: Vec<String>,
    tag_ids: Vec<String>,
    people_details: Vec<NhentaiRelation>,
    tag_details: Vec<NhentaiRelation>,
    parody_details: Vec<NhentaiRelation>,
}

fn parse_gallery_title(document: &Html) -> String {
    let title_selectors = ["#info h1.title", "h1.title", "meta[property=\"og:title\"]"];

    for selector_str in title_selectors {
        let selector = Selector::parse(selector_str).expect("valid title selector");
        if let Some(node) = document.select(&selector).next() {
            let value = if selector_str.starts_with("meta[") {
                node.value().attr("content").unwrap_or_default().to_string()
            } else {
                node.text().collect::<String>()
            };
            let value = value.trim_end_matches(" - nhentai");
            let normalized = normalize_text(value);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }

    String::new()
}

fn parse_gallery_cover_url(document: &Html) -> String {
    let img_selector = Selector::parse("#cover img, img#cover").expect("valid cover img selector");
    if let Some(img) = document.select(&img_selector).next() {
        if let Some(src) = img
            .value()
            .attr("data-src")
            .or_else(|| img.value().attr("src"))
        {
            let normalized = normalize_url(src);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }

    let meta_selector =
        Selector::parse("meta[property=\"og:image\"]").expect("valid og:image selector");
    document
        .select(&meta_selector)
        .next()
        .and_then(|meta| meta.value().attr("content"))
        .map(normalize_url)
        .unwrap_or_default()
}

fn parse_tag_buckets(document: &Html) -> TagBuckets {
    let container_selector =
        Selector::parse("#tags .tag-container, .tag-container").expect("valid tag selector");
    let tag_selector = Selector::parse("a.tag").expect("valid tag selector");
    let tag_name_selector = Selector::parse("span.name").expect("valid tag name selector");

    let mut out = TagBuckets::default();

    for container in document.select(&container_selector) {
        let Some(label) = parse_tag_container_label(&container) else {
            continue;
        };

        let mut values = Vec::new();
        let mut relation_ids = Vec::new();
        let mut relation_details: Vec<NhentaiRelation> = Vec::new();
        let fallback_key = relation_key_for_label(&label);

        for anchor in container.select(&tag_selector) {
            let mut value = anchor
                .select(&tag_name_selector)
                .next()
                .map(|el| normalize_text(&el.text().collect::<String>()))
                .unwrap_or_default();

            if value.is_empty() {
                value = normalize_text(&anchor.text().collect::<String>());
            }

            if !value.is_empty() && !values.iter().any(|existing| existing == &value) {
                values.push(value.clone());
            }

            let href = anchor.value().attr("href").unwrap_or_default();
            let key = parse_relation_key_from_href(href)
                .or_else(|| fallback_key.map(str::to_string))
                .unwrap_or_default();
            if key.is_empty() {
                continue;
            }

            let slug = parse_relation_slug_from_href(href).or_else(|| slugify_identifier(&value));
            let Some(slug) = slug else {
                continue;
            };

            let relation_id = format!("nhentai-{key}:{slug}");
            if !relation_ids.iter().any(|existing| existing == &relation_id) {
                relation_ids.push(relation_id.clone());
            }

            let relation_name = if value.is_empty() {
                slug.replace('-', " ")
            } else {
                value.clone()
            };
            if !relation_name.is_empty() && !relation_details.iter().any(|r| r.id == relation_id) {
                relation_details.push(NhentaiRelation {
                    id: relation_id,
                    name: relation_name,
                });
            }
        }

        if values.is_empty() && relation_ids.is_empty() {
            continue;
        }

        if label == "tags" {
            push_all_unique(&mut out.tags, values);
            push_all_unique(&mut out.tag_ids, relation_ids);
            push_all_unique_relations(&mut out.tag_details, relation_details);
            continue;
        }

        if label == "artists" || label == "artist" {
            push_all_unique(&mut out.artists, values);
            push_all_unique(&mut out.people_ids, relation_ids);
            push_all_unique_relations(&mut out.people_details, relation_details);
            continue;
        }

        if label == "groups" || label == "group" {
            push_all_unique(&mut out.groups, values);
            push_all_unique(&mut out.people_ids, relation_ids);
            push_all_unique_relations(&mut out.people_details, relation_details);
            continue;
        }

        if label == "parodies" || label == "parody" {
            push_all_unique(&mut out.parodies, values);
            push_all_unique_relations(&mut out.parody_details, relation_details);
            continue;
        }

        if label == "characters" || label == "character" {
            push_all_unique(&mut out.characters, values);
            push_all_unique(&mut out.people_ids, relation_ids);
            push_all_unique_relations(&mut out.people_details, relation_details);
            continue;
        }

        if label == "languages" || label == "language" {
            push_all_unique(&mut out.languages, values);
            push_all_unique(&mut out.tag_ids, relation_ids);
            push_all_unique_relations(&mut out.tag_details, relation_details);
            continue;
        }

        if label == "categories" || label == "category" {
            push_all_unique(&mut out.categories, values);
            push_all_unique(&mut out.tag_ids, relation_ids);
            push_all_unique_relations(&mut out.tag_details, relation_details);
            continue;
        }

        if label == "pages" {
            out.pages = values.first().and_then(|value| parse_u32_from_text(value));
        }
    }

    out
}

fn relation_key_for_label(label: &str) -> Option<&'static str> {
    match label {
        "tags" | "tag" => Some("tags"),
        "artists" | "artist" => Some("artist"),
        "groups" | "group" => Some("group"),
        "parodies" | "parody" => Some("parody"),
        "characters" | "character" => Some("character"),
        "languages" | "language" => Some("language"),
        "categories" | "category" => Some("category"),
        _ => None,
    }
}

fn parse_relation_key_from_href(href: &str) -> Option<String> {
    let normalized = normalize_url(href);
    let path = normalized
        .strip_prefix("https://nhentai.net/")
        .or_else(|| normalized.strip_prefix("http://nhentai.net/"))
        .or_else(|| normalized.strip_prefix("https://www.nhentai.net/"))
        .or_else(|| normalized.strip_prefix("http://www.nhentai.net/"))
        .unwrap_or(normalized.as_str());
    let cleaned = path
        .split_once('?')
        .map(|(left, _)| left)
        .unwrap_or(path)
        .split_once('#')
        .map(|(left, _)| left)
        .unwrap_or(path);

    let mut segments = cleaned
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty());

    let key = segments.next()?.to_ascii_lowercase();
    if key == "tag" {
        Some("tags".to_string())
    } else {
        Some(key)
    }
}

fn parse_relation_slug_from_href(href: &str) -> Option<String> {
    let normalized = normalize_url(href);
    let path = normalized
        .strip_prefix("https://nhentai.net/")
        .or_else(|| normalized.strip_prefix("http://nhentai.net/"))
        .or_else(|| normalized.strip_prefix("https://www.nhentai.net/"))
        .or_else(|| normalized.strip_prefix("http://www.nhentai.net/"))
        .unwrap_or(normalized.as_str());
    let cleaned = path
        .split_once('?')
        .map(|(left, _)| left)
        .unwrap_or(path)
        .split_once('#')
        .map(|(left, _)| left)
        .unwrap_or(path);

    cleaned
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .next_back()
        .map(|segment| segment.to_ascii_lowercase())
}

fn slugify_identifier(value: &str) -> Option<String> {
    let mut slug = String::new();
    let mut prev_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_tag_container_label(container: &ElementRef<'_>) -> Option<String> {
    for child in container.children() {
        if let Some(text) = child.value().as_text() {
            let normalized = normalize_text(text)
                .trim_end_matches(':')
                .to_ascii_lowercase();
            if !normalized.is_empty() {
                return Some(normalized);
            }
        }

        let Some(element) = ElementRef::wrap(child) else {
            continue;
        };
        if element.value().name() != "span" {
            continue;
        }

        if !element
            .value()
            .classes()
            .any(|class_name| class_name == "name")
        {
            continue;
        }

        let raw = element.text().collect::<String>();
        let normalized = normalize_text(&raw)
            .trim_end_matches(':')
            .to_ascii_lowercase();
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }

    None
}

fn parse_script_image_urls(document: &Html) -> Option<Vec<String>> {
    let script_selector = Selector::parse("script").expect("valid script selector");
    let media_re = Regex::new(r#"(?s)(?:\\?"media_id\\?"\s*:\s*\\?"(?P<id>\d+)\\?")"#)
        .expect("valid media regex");
    let pages_re =
        Regex::new(r#"(?s)(?:\\?"pages\\?"\s*:\s*\[(?P<pages>.*?)\])"#).expect("valid pages regex");
    let page_type_re = Regex::new(r#"(?s)(?:\\?"t\\?"\s*:\s*\\?"(?P<t>[a-z])\\?")"#)
        .expect("valid page type regex");

    for script in document.select(&script_selector) {
        let body = script.text().collect::<String>();
        if !body.contains("media_id") {
            continue;
        }

        let Some(media_caps) = media_re.captures(&body) else {
            continue;
        };
        let Some(media_id_match) = media_caps.name("id") else {
            continue;
        };
        let media_id = media_id_match.as_str();

        let Some(pages_caps) = pages_re.captures(&body) else {
            continue;
        };
        let Some(pages_blob) = pages_caps.name("pages") else {
            continue;
        };

        let page_types = page_type_re
            .captures_iter(pages_blob.as_str())
            .filter_map(|caps| caps.name("t").map(|m| m.as_str().to_string()))
            .collect::<Vec<_>>();

        if page_types.is_empty() {
            continue;
        }

        let urls = page_types
            .iter()
            .enumerate()
            .map(|(idx, page_type)| {
                format!(
                    "https://i.nhentai.net/galleries/{media_id}/{}.{}",
                    idx + 1,
                    image_extension(page_type)
                )
            })
            .collect::<Vec<_>>();

        return Some(urls);
    }

    None
}

fn parse_thumbnail_image_urls(document: &Html) -> Vec<String> {
    let thumb_selector = Selector::parse("#thumbnail-container img, .thumb-container img")
        .expect("valid thumb selector");

    let mut out = Vec::new();

    for image in document.select(&thumb_selector) {
        let Some(src) = image
            .value()
            .attr("data-src")
            .or_else(|| image.value().attr("src"))
        else {
            continue;
        };

        let normalized = normalize_url(src);
        if normalized.is_empty() {
            continue;
        }

        let full = thumbnail_to_image_url(&normalized).unwrap_or(normalized);
        if !out.iter().any(|existing| existing == &full) {
            out.push(full);
        }
    }

    out
}

fn thumbnail_to_image_url(thumbnail_url: &str) -> Option<String> {
    let thumb_re = Regex::new(
        r#"^https?://(?:t\d*|t)\.nhentai\.net/galleries/(?P<gallery>\d+)/(?P<page>\d+)t\.(?P<ext>jpg|png|gif|webp)(?:\.\w+)?$"#,
    )
    .expect("valid thumbnail regex");

    let caps = thumb_re.captures(thumbnail_url)?;
    let gallery = caps.name("gallery")?.as_str();
    let page = caps.name("page")?.as_str();
    let ext = caps.name("ext")?.as_str();

    Some(format!(
        "https://i.nhentai.net/galleries/{gallery}/{page}.{ext}"
    ))
}

fn image_extension(token: &str) -> &str {
    match token {
        "p" => "png",
        "g" => "gif",
        "w" => "webp",
        _ => "jpg",
    }
}

fn parse_u32_from_text(value: &str) -> Option<u32> {
    let digits = value
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();

    if digits.is_empty() {
        None
    } else {
        digits.parse::<u32>().ok()
    }
}

fn push_all_unique(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !target.iter().any(|existing| existing == &value) {
            target.push(value);
        }
    }
}

fn push_all_unique_relations(target: &mut Vec<NhentaiRelation>, values: Vec<NhentaiRelation>) {
    for value in values {
        if !target.iter().any(|existing| existing.id == value.id) {
            target.push(value);
        }
    }
}

fn deduplicate_strings(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();

    for value in values {
        if !out.iter().any(|existing| existing == &value) {
            out.push(value);
        }
    }

    out
}

fn is_valid_gallery_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return trimmed.to_string();
    }

    if let Some(rest) = trimmed.strip_prefix("//") {
        return format!("https://{rest}");
    }

    if trimmed.starts_with('/') {
        return format!("https://nhentai.net{trimmed}");
    }

    format!("https://nhentai.net/{trimmed}")
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*b as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{:02X}", b)),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_search_url_adds_english_prefix() {
        let url = build_search_url("soft", None).expect("url");
        assert_eq!(url, "https://nhentai.net/search/?q=language%3Aenglish+soft");
    }

    #[test]
    fn build_search_url_appends_page() {
        let url = build_search_url("soft", Some(3)).expect("url");
        assert_eq!(
            url,
            "https://nhentai.net/search/?q=language%3Aenglish+soft&page=3"
        );
    }

    #[test]
    fn build_search_url_page_one_omits_param() {
        let url = build_search_url("soft", Some(1)).expect("url");
        assert_eq!(url, "https://nhentai.net/search/?q=language%3Aenglish+soft");
    }

    #[test]
    fn parse_lookup_gallery_id_supports_prefix_and_url() {
        assert_eq!(
            parse_lookup_gallery_id("nhentai:12345"),
            Some("12345".to_string())
        );
        assert_eq!(
            parse_lookup_gallery_id("https://nhentai.net/g/67890/"),
            Some("67890".to_string())
        );
        assert_eq!(parse_lookup_gallery_id("soft sample"), None);
    }

    #[test]
    fn parse_search_html_reads_title_cover_and_id() {
        let html = r#"
        <div class="gallery">
          <a class="cover" href="/g/12345/">
            <img data-src="//t3.nhentai.net/galleries/111/thumb.jpg" />
          </a>
          <div class="caption">Soft Sample</div>
        </div>
        <div class="gallery">
          <a class="cover" href="/g/67890/">
            <img src="/galleries/222/thumb.jpg" />
          </a>
          <div class="caption">Another Work</div>
        </div>
        "#;

        let results = parse_search_html(html);
        assert_eq!(results.len(), 2);

        assert_eq!(results[0].id, Some("12345".to_string()));
        assert_eq!(results[0].title, "Soft Sample");
        assert_eq!(
            results[0].cover_url,
            "https://t3.nhentai.net/galleries/111/thumb.jpg"
        );
        assert_eq!(results[0].gallery_url, "https://nhentai.net/g/12345/");

        assert_eq!(results[1].id, Some("67890".to_string()));
        assert_eq!(results[1].title, "Another Work");
        assert_eq!(
            results[1].cover_url,
            "https://nhentai.net/galleries/222/thumb.jpg"
        );
        assert_eq!(results[1].gallery_url, "https://nhentai.net/g/67890/");
    }

    #[test]
    fn parse_search_html_skips_invalid_rows() {
        let html = r#"
        <div class="gallery">
          <a class="cover" href="/g/12345/"></a>
          <div class="caption">No Image</div>
        </div>
        <div class="gallery">
          <a class="cover" href="/g/98765/">
            <img src="/galleries/333/thumb.jpg" />
          </a>
        </div>
        "#;

        let results = parse_search_html(html);
        assert!(results.is_empty());
    }

    #[test]
    fn parse_gallery_html_extracts_rich_metadata_and_images() {
        let html = r#"
        <html>
          <head>
            <meta property="og:image" content="https://t.nhentai.net/galleries/555/cover.jpg" />
          </head>
          <body>
            <div id="info">
              <h1 class="title">Sample Gallery</h1>
            </div>
            <div id="tags">
              <div class="tag-container field-name">
                <span class="name">Artists:</span>
                <span class="tags"><a class="tag" href="/artist/artist-one/"><span class="name">artist-one</span></a></span>
              </div>
              <div class="tag-container field-name">
                <span class="name">Groups:</span>
                <span class="tags"><a class="tag" href="/group/group-one/"><span class="name">group-one</span></a></span>
              </div>
              <div class="tag-container field-name">
                <span class="name">Tags:</span>
                <span class="tags"><a class="tag" href="/tag/full-color/"><span class="name">full color</span></a></span>
              </div>
              <div class="tag-container field-name">
                <span class="name">Languages:</span>
                <span class="tags"><a class="tag" href="/language/english/"><span class="name">english</span></a></span>
              </div>
              <div class="tag-container field-name">
                <span class="name">Categories:</span>
                <span class="tags"><a class="tag" href="/category/doujinshi/"><span class="name">doujinshi</span></a></span>
              </div>
              <div class="tag-container field-name">
                <span class="name">Pages:</span>
                <span class="tags"><a class="tag"><span class="name">24</span></a></span>
              </div>
            </div>
            <script>
              window._gallery = {"media_id":"555","images":{"pages":[{"t":"j"},{"t":"p"}]}};
            </script>
          </body>
        </html>
        "#;

        let result = parse_gallery_html(html, "12345").expect("gallery should parse");
        assert_eq!(result.id, Some("12345".to_string()));
        assert_eq!(result.title, "Sample Gallery");
        assert_eq!(result.gallery_url, "https://nhentai.net/g/12345/");
        assert_eq!(
            result.cover_url,
            "https://t.nhentai.net/galleries/555/cover.jpg"
        );
        assert_eq!(result.artists, vec!["artist-one".to_string()]);
        assert_eq!(result.groups, vec!["group-one".to_string()]);
        assert_eq!(result.tags, vec!["full color".to_string()]);
        assert_eq!(result.languages, vec!["english".to_string()]);
        assert_eq!(result.categories, vec!["doujinshi".to_string()]);
        assert_eq!(
            result.people_ids,
            vec![
                "nhentai-artist:artist-one".to_string(),
                "nhentai-group:group-one".to_string()
            ]
        );
        assert_eq!(
            result.people_details,
            vec![
                NhentaiRelation {
                    id: "nhentai-artist:artist-one".to_string(),
                    name: "artist-one".to_string()
                },
                NhentaiRelation {
                    id: "nhentai-group:group-one".to_string(),
                    name: "group-one".to_string()
                }
            ]
        );
        assert_eq!(
            result.tag_ids,
            vec![
                "nhentai-tags:full-color".to_string(),
                "nhentai-language:english".to_string(),
                "nhentai-category:doujinshi".to_string()
            ]
        );
        assert_eq!(
            result.tag_details,
            vec![
                NhentaiRelation {
                    id: "nhentai-tags:full-color".to_string(),
                    name: "full color".to_string()
                },
                NhentaiRelation {
                    id: "nhentai-language:english".to_string(),
                    name: "english".to_string()
                },
                NhentaiRelation {
                    id: "nhentai-category:doujinshi".to_string(),
                    name: "doujinshi".to_string()
                }
            ]
        );
        assert_eq!(result.pages, Some(24));
        assert_eq!(
            result.images,
            vec![
                "https://i.nhentai.net/galleries/555/1.jpg".to_string(),
                "https://i.nhentai.net/galleries/555/2.png".to_string()
            ]
        );
    }

    #[test]
    fn parse_gallery_html_uses_thumbnail_fallback() {
        let html = r#"
        <html>
          <body>
            <h1 class="title">Fallback Gallery</h1>
            <div id="thumbnail-container">
              <img data-src="https://t5.nhentai.net/galleries/987/1t.jpg" />
            </div>
          </body>
        </html>
        "#;

        let result = parse_gallery_html(html, "987").expect("gallery should parse");
        assert_eq!(
            result.images,
            vec!["https://i.nhentai.net/galleries/987/1.jpg".to_string()]
        );
    }

    #[test]
    fn parse_gallery_html_thumbnail_fallback_handles_double_extension() {
        let html = r#"
        <html>
          <body>
            <h1 class="title">Double Ext Gallery</h1>
            <div id="thumbnail-container">
              <img data-src="https://t4.nhentai.net/galleries/123/1t.webp" />
              <img data-src="https://t4.nhentai.net/galleries/123/2t.webp.webp" />
            </div>
          </body>
        </html>
        "#;

        let result = parse_gallery_html(html, "123").expect("gallery should parse");
        assert_eq!(
            result.images,
            vec![
                "https://i.nhentai.net/galleries/123/1.webp".to_string(),
                "https://i.nhentai.net/galleries/123/2.webp".to_string(),
            ]
        );
    }

    #[test]
    fn parse_gallery_html_reads_escaped_script_json() {
        let html = r#"
        <html>
          <body>
            <h1 class="title">Escaped Script</h1>
            <script>
              window._n_app = JSON.parse("{\"media_id\":\"700\",\"images\":{\"pages\":[{\"t\":\"j\"},{\"t\":\"w\"}]}}");
            </script>
          </body>
        </html>
        "#;

        let result = parse_gallery_html(html, "700").expect("gallery should parse");
        assert_eq!(
            result.images,
            vec![
                "https://i.nhentai.net/galleries/700/1.jpg".to_string(),
                "https://i.nhentai.net/galleries/700/2.webp".to_string()
            ]
        );
    }

    #[test]
    fn parse_gallery_html_reads_artist_from_plain_text_label() {
        let html = r#"
        <html>
          <body>
            <h1 class="title">Plain Text Label</h1>
            <div id="tags">
              <div class="tag-container field-name ">
                Artists:
                <span class="tags">
                  <a href="/artist/bai-asuka/" class="tag tag-32383 ">
                    <span class="name">bai asuka</span>
                    <span class="count">574</span>
                  </a>
                </span>
              </div>
            </div>
          </body>
        </html>
        "#;

        let result = parse_gallery_html(html, "629637").expect("gallery should parse");
        assert_eq!(result.artists, vec!["bai asuka".to_string()]);
        assert_eq!(
            result.people_ids,
            vec!["nhentai-artist:bai-asuka".to_string()]
        );
        assert_eq!(
            result.people_details,
            vec![NhentaiRelation {
                id: "nhentai-artist:bai-asuka".to_string(),
                name: "bai asuka".to_string()
            }]
        );
    }

    #[test]
    fn parse_gallery_html_reads_parody_into_parody_details() {
        let html = r#"
        <html>
          <body>
            <h1 class="title">Parody Work</h1>
            <div id="tags">
              <div class="tag-container field-name">
                Parodies:
                <span class="tags">
                  <a href="/parody/naruto/" class="tag">
                    <span class="name">naruto</span>
                  </a>
                </span>
              </div>
            </div>
          </body>
        </html>
        "#;

        let result = parse_gallery_html(html, "282849").expect("gallery should parse");
        assert_eq!(result.parodies, vec!["naruto".to_string()]);
        assert_eq!(
            result.parody_details,
            vec![NhentaiRelation {
                id: "nhentai-parody:naruto".to_string(),
                name: "naruto".to_string()
            }]
        );
        assert!(result.tag_details.is_empty());
        assert!(result.tag_ids.is_empty());
    }
}
