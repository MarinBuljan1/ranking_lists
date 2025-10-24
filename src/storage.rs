use crate::ranking::BradleyTerry;
use gloo_storage::{LocalStorage, Storage};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const STORAGE_KEY: &str = "ranking_lists_state";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredAppState {
    pub selected_list: Option<String>,
    pub lists: HashMap<String, StoredListState>,
}

impl Default for StoredAppState {
    fn default() -> Self {
        Self {
            selected_list: None,
            lists: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StoredListState {
    pub ratings: HashMap<String, f64>,
    pub match_history: Vec<MatchRecord>,
}

impl Default for StoredListState {
    fn default() -> Self {
        Self {
            ratings: HashMap::new(),
            match_history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRecord {
    pub winner_id: String,
    pub loser_id: String,
    #[serde(default)]
    pub timestamp_ms: Option<u64>,
}

pub fn load_state() -> StoredAppState {
    match LocalStorage::get::<StoredAppState>(STORAGE_KEY) {
        Ok(state) => state,
        Err(err) => {
            warn!("Falling back to default app state: {}", err);
            StoredAppState::default()
        }
    }
}

pub fn save_state(state: &StoredAppState) {
    if let Err(err) = LocalStorage::set(STORAGE_KEY, state) {
        warn!("Failed to persist state: {}", err);
    }
}

pub fn load_list_state(list_id: &str) -> StoredListState {
    load_state()
        .lists
        .get(list_id)
        .cloned()
        .unwrap_or_default()
}

pub fn save_list_state(list_id: &str, list_state: StoredListState) {
    let mut state = load_state();
    state.lists.insert(list_id.to_string(), list_state);
    save_state(&state);
}

pub fn load_ranking(list_id: &str, k_factor: f64) -> BradleyTerry {
    let stored = load_list_state(list_id);
    BradleyTerry::from_ratings(stored.ratings, k_factor)
}

pub fn save_ranking(list_id: &str, ranking: &BradleyTerry) {
    let mut state = load_state();
    let entry = state
        .lists
        .entry(list_id.to_string())
        .or_insert_with(StoredListState::default);
    entry.ratings = ranking.ratings().clone();
    save_state(&state);
}
