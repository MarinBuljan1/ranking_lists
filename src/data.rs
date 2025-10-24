use gloo_net::http::Request;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedList {
    pub info: ListInfo,
    pub items: Vec<ListItem>,
}

#[derive(Debug)]
pub enum DataError {
    NotFound(String),
    Network(String),
    Parse(String),
}

impl DataError {
    fn network<E: std::fmt::Display>(err: E) -> Self {
        Self::Network(err.to_string())
    }

    fn parse<E: std::fmt::Display>(err: E) -> Self {
        Self::Parse(err.to_string())
    }
}

pub async fn fetch_available_lists() -> Result<Vec<ListInfo>, DataError> {
    let response = Request::get("assets/index.json")
        .send()
        .await
        .map_err(DataError::network)?;

    if !response.ok() {
        return Err(DataError::Network(format!(
            "HTTP {} while fetching list index",
            response.status()
        )));
    }

    let text = response.text().await.map_err(DataError::network)?;
    let ids: Vec<String> =
        serde_json::from_str(&text).map_err(DataError::parse)?;

    let infos = ids
        .into_iter()
        .map(|id| ListInfo {
            label: display_name(&id),
            id,
        })
        .collect();

    Ok(infos)
}

pub async fn load_list(list_id: &str) -> Result<LoadedList, DataError> {
    let url = format!("assets/lists/{}.json", list_id);
    let response = Request::get(&url)
        .send()
        .await
        .map_err(DataError::network)?;

    if response.status() == 404 {
        return Err(DataError::NotFound(list_id.to_owned()));
    }

    if !response.ok() {
        return Err(DataError::Network(format!(
            "HTTP {} while fetching {}",
            response.status(),
            url
        )));
    }

    let text = response.text().await.map_err(DataError::network)?;
    let raw_items: Vec<String> =
        serde_json::from_str(&text).map_err(DataError::parse)?;

    if raw_items.is_empty() {
        return Err(DataError::Parse(format!(
            "List '{}' does not contain any items",
            list_id
        )));
    }

    let mut seen = HashSet::new();
    let mut items = Vec::with_capacity(raw_items.len());

    for (index, label) in raw_items.into_iter().enumerate() {
        let trimmed = label.trim().to_string();
        if trimmed.is_empty() {
            return Err(DataError::Parse(format!(
                "Item {} in list '{}' is empty",
                index, list_id
            )));
        }

        let mut candidate = slugify(&trimmed);
        if candidate.is_empty() {
            candidate = format!("item-{}", index);
        }

        let id = ensure_unique_id(&mut seen, candidate);
        items.push(ListItem { id, label: trimmed });
    }

    Ok(LoadedList {
        info: ListInfo {
            id: list_id.to_owned(),
            label: display_name(list_id),
        },
        items,
    })
}

fn ensure_unique_id(seen: &mut HashSet<String>, base: String) -> String {
    if seen.insert(base.clone()) {
        return base;
    }

    let mut counter = 2;
    loop {
        let candidate = format!("{}-{}", base, counter);
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

fn display_name(id: &str) -> String {
    id.split(|c: char| c == '_' || c == '-' || c == ' ')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>()
                    + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || matches!(ch, '-' | '_') {
            if !slug.ends_with('-') {
                slug.push('-');
            }
        }
    }

    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Green Apple"), "green-apple");
        assert_eq!(slugify("  Mango!!!  "), "mango");
    }

    #[test]
    fn display_name_basic() {
        assert_eq!(display_name("citrus-fruits"), "Citrus Fruits");
        assert_eq!(display_name("stone_fruit"), "Stone Fruit");
    }
}
