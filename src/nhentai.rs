use scraper::{Html, Selector};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NhentaiGallery {
    pub id: Option<String>,
    pub title: String,
    pub cover_url: String,
    pub gallery_url: String,
}

pub fn build_search_url(search: &str) -> Option<String> {
    let trimmed = search.trim();
    if trimmed.is_empty() {
        return None;
    }

    let query = format!("language:english {trimmed}");
    Some(format!(
        "https://nhentai.net/search/?q={}",
        encode_query_component(&query)
    ))
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
            .map(|el| el.text().collect::<String>().trim().to_string())
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
            cover_url,
            gallery_url,
        });
    }

    items
}

pub fn extract_gallery_id(href: &str) -> Option<String> {
    let path = href.trim().trim_matches('/');
    let mut parts = path.split('/');

    let section = parts.next()?;
    let id = parts.next()?;

    if section == "g" && !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()) {
        Some(id.to_string())
    } else {
        None
    }
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
        let url = build_search_url("soft").expect("url");
        assert_eq!(url, "https://nhentai.net/search/?q=language%3Aenglish+soft");
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
}
