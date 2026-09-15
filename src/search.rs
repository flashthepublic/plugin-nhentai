use rs_plugin_common_interfaces::{
    domain::{person::PersonType, rs_ids::RsIds},
    lookup::RsLookupBook,
};

use crate::nhentai::parse_relation_search_term;

/// Combine constraints with AND. Unresolvable constraints must not silently
/// become a broader search. Native relation IDs take precedence over names.
pub fn book_search(book: &RsLookupBook, base: Option<String>) -> Option<String> {
    let mut terms: Vec<String> = base.into_iter().collect();
    if let Some(author) = book.author.as_deref().filter(|v| !v.trim().is_empty()) {
        terms.push(filter_term(Some(author), None, &["artist"], "artist")?);
    }
    for person in book.people.iter().flatten() {
        let (categories, fallback): (&[&str], &str) = match person.role.as_ref() {
            None => (&["artist", "group", "character"], ""),
            Some(PersonType::Author) => (&["artist"], "artist"),
            Some(PersonType::Character) => (&["character"], "character"),
            Some(PersonType::Custom(role)) if role == "artist" => (&["artist"], "artist"),
            Some(PersonType::Custom(role)) if role == "group" => (&["group"], "group"),
            Some(PersonType::Custom(role)) if role == "character" => (&["character"], "character"),
            _ => return None,
        };
        terms.push(filter_term(
            person.name.as_deref(),
            person.ids.as_ref(),
            categories,
            fallback,
        )?);
    }
    for series in book.series.iter().flatten() {
        terms.push(filter_term(
            series.name.as_deref(),
            series.ids.as_ref(),
            &["parody"],
            "parody",
        )?);
    }
    for tag in book.tags.iter().flatten() {
        terms.push(filter_term(
            tag.name.as_deref(),
            tag.ids.as_ref(),
            &["tag", "language", "category"],
            "tag",
        )?);
    }
    let mut seen = std::collections::HashSet::new();
    terms.retain(|term| !term.trim().is_empty() && seen.insert(term.clone()));
    (!terms.is_empty()).then(|| terms.join(" "))
}

fn filter_term(
    name: Option<&str>,
    ids: Option<&RsIds>,
    categories: &[&str],
    fallback: &str,
) -> Option<String> {
    let native = ids
        .into_iter()
        .flat_map(RsIds::as_all_ids)
        .find_map(|id| parse_relation_search_term(&id))
        .or_else(|| name.and_then(parse_relation_search_term));
    if let Some(term) = native {
        let (category, value) = term.split_once(':')?;
        if !categories.contains(&category) {
            return None;
        }
        // Slugs supplied by nhentai already use hyphens for spaces.
        return Some(format!("{category}:{}", quote_value(value)?));
    }
    let value = quote_value(name?)?;
    Some(if fallback.is_empty() {
        value
    } else {
        format!("{fallback}:{value}")
    })
}

fn quote_value(value: &str) -> Option<String> {
    let value = value.trim();
    // Quotes/control characters cannot be represented safely as a search literal.
    if value.is_empty()
        || value
            .chars()
            .any(|c| c == '"' || c == '\\' || c.is_control())
    {
        return None;
    }
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && !value.starts_with('-')
    {
        Some(value.to_string())
    } else {
        Some(format!("\"{value}\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn combines_filters_without_title_and_preserves_namespaces() {
        let book: RsLookupBook = serde_json::from_value(json!({
            "people": [{"name":"ignored display name", "ids":{"nhentai-group":"sample-group"}}],
            "series": [{"name": "sample series"}],
            "tags": [{"name": "full color"}, {"name": "nhentai-language:japanese"}]
        }))
        .unwrap();
        assert_eq!(
            book_search(&book, None).as_deref(),
            Some(
                "group:sample-group parody:\"sample series\" tag:\"full color\" language:japanese"
            )
        );
    }

    #[test]
    fn combines_title_author_and_person_roles() {
        let book: RsLookupBook = serde_json::from_value(json!({
            "author": "sample artist", "people": [
                {"name": "sample character", "role": "Character"},
                {"name": "sample group", "role": "group"},
                {"name": "another person"}
            ], "tags": [{"name": "color"}]
        }))
        .unwrap();
        assert_eq!(book_search(&book, Some("title".into())).unwrap(),
            "title artist:\"sample artist\" character:\"sample character\" group:\"sample group\" \"another person\" tag:color");
    }

    #[test]
    fn rejects_unsupported_roles_ids_and_invalid_literals() {
        for filter in [
            json!({"role":"Director", "name":"someone"}),
            json!({"role":"Author", "name":"nhentai-character:someone"}),
            json!({"ids":{"imdb":"nm123"}}),
            json!({"name":"bad\" name"}),
        ] {
            let book = serde_json::from_value(json!({"people":[filter]})).unwrap();
            assert!(book_search(&book, Some("title".into())).is_none());
        }
    }

    #[test]
    fn native_ids_round_trip_for_each_filter_category() {
        for (field, prefix, category) in [
            ("people", "nhentai-artist", "artist"),
            ("people", "nhentai-character", "character"),
            ("series", "nhentai-parody", "parody"),
            ("tags", "nhentai-tags", "tag"),
            ("tags", "nhentai-category", "category"),
        ] {
            let book = serde_json::from_value(json!({
                (field): [{"name":"ignored", "ids":{(prefix): "sample"}}]
            }))
            .unwrap();
            assert_eq!(
                book_search(&book, None).unwrap(),
                format!("{category}:sample")
            );
        }
        let book = serde_json::from_value(json!({
            "tags":[{"name":"color", "ids":{"wikidata":"Q123"}}]
        }))
        .unwrap();
        assert_eq!(book_search(&book, None).unwrap(), "tag:color");
    }

    #[test]
    fn old_and_empty_queries_remain_supported() {
        assert_eq!(
            book_search(&RsLookupBook::default(), Some("title".into())).as_deref(),
            Some("title")
        );
        assert!(book_search(&RsLookupBook::default(), None).is_none());
    }
}
