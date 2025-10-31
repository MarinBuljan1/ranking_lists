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
pub struct StoredListState {
    pub item_ids: Vec<String>,
    pub win_matrix: Vec<Vec<u32>>,
    pub abilities: Vec<f64>,
    #[serde(default)]
    pub match_totals: Vec<u32>,
}

impl StoredListState {
    pub fn new(item_ids: &[String]) -> Self {
        let count = item_ids.len();
        Self {
            item_ids: item_ids.to_vec(),
            win_matrix: vec![vec![0; count]; count],
            abilities: vec![1.0; count],
            match_totals: vec![0; count],
        }
    }

    pub fn matches_items(&self, item_ids: &[String]) -> bool {
        self.item_ids == item_ids
            && self.win_matrix.len() == item_ids.len()
            && self
                .win_matrix
                .iter()
                .all(|row| row.len() == item_ids.len())
            && self.abilities.len() == item_ids.len()
    }

    pub fn total_matches(&self) -> u32 {
        if self.match_totals.len() == self.item_ids.len() {
            self.match_totals.iter().copied().sum::<u32>() / 2
        } else {
            self.win_matrix
                .iter()
                .map(|row| row.iter().sum::<u32>())
                .sum()
        }
    }
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

pub fn load_list_state<'a>(
    app_state: &'a StoredAppState,
    list_id: &str,
) -> Option<&'a StoredListState> {
    app_state.lists.get(list_id)
}

pub fn upsert_list_state(app_state: &mut StoredAppState, list_id: &str, state: StoredListState) {
    app_state.lists.insert(list_id.to_string(), state);
}

pub fn align_list_state(existing: Option<StoredListState>, item_ids: &[String]) -> StoredListState {
    match existing {
        Some(mut state) if state.matches_items(item_ids) => {
            if state.match_totals.len() != state.item_ids.len() {
                state.match_totals = compute_match_totals(&state.win_matrix);
            }
            state
        }
        Some(state) => reorder_state(state, item_ids),
        None => StoredListState::new(item_ids),
    }
}

fn reorder_state(state: StoredListState, item_ids: &[String]) -> StoredListState {
    let n = item_ids.len();
    if n == 0 {
        return StoredListState::new(item_ids);
    }

    let mut mapping = HashMap::new();
    for (idx, id) in state.item_ids.iter().enumerate() {
        mapping.insert(id.clone(), idx);
    }

    let mut new_state = StoredListState::new(item_ids);

    for (new_i, id_i) in item_ids.iter().enumerate() {
        if let Some(&old_i) = mapping.get(id_i) {
            if old_i < state.abilities.len() {
                new_state.abilities[new_i] = state.abilities[old_i].max(1e-6);
            }
            for (new_j, id_j) in item_ids.iter().enumerate() {
                if let Some(&old_j) = mapping.get(id_j) {
                    let value = state
                        .win_matrix
                        .get(old_i)
                        .and_then(|row| row.get(old_j))
                        .copied()
                        .unwrap_or(0);
                    new_state.win_matrix[new_i][new_j] = value;
                }
            }
        }
    }
    new_state.match_totals = compute_match_totals(&new_state.win_matrix);

    new_state
}

fn compute_match_totals(win_matrix: &[Vec<u32>]) -> Vec<u32> {
    let count = win_matrix.len();
    let mut totals = vec![0u32; count];
    for i in 0..count {
        for j in 0..count {
            if i == j {
                continue;
            }
            let matches = win_matrix[i].get(j).copied().unwrap_or(0)
                + win_matrix
                    .get(j)
                    .and_then(|row| row.get(i))
                    .copied()
                    .unwrap_or(0);
            totals[i] = totals[i].saturating_add(matches);
        }
    }
    totals
}
