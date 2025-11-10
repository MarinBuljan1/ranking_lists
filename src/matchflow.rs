use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;

#[derive(Debug, Clone)]
pub struct Matchup {
    pub left_index: usize,
    pub right_index: usize,
}

const TOP_BIAS_POWER: f64 = 0.15;
const PROXIMITY_ALPHA: f64 = 4.0;
const RECENT_PAIR_PENALTY: f64 = 0.35;
const MIN_WEIGHT: f64 = 1e-9;

pub fn random_matchup(
    abilities: &[f64],
    win_matrix: &[Vec<u32>],
    match_totals: &[u32],
    last: Option<&Matchup>,
) -> Option<Matchup> {
    let count = abilities.len().min(win_matrix.len());
    if count < 2 {
        return None;
    }

    let mut rng = rand::thread_rng();

    // Bias first selection toward higher-rated items and those with fewer total matches.
    let total_ability: f64 = abilities.iter().copied().sum::<f64>().max(MIN_WEIGHT);
    let mut first_weights = Vec::with_capacity(count);
    for i in 0..count {
        let ability_bias = (abilities[i].max(MIN_WEIGHT) / total_ability).powf(TOP_BIAS_POWER);
        let total_matches = match_totals.get(i).copied().unwrap_or_else(|| {
            win_matrix[i].iter().copied().sum::<u32>()
                + win_matrix
                    .iter()
                    .enumerate()
                    .filter_map(|(row_idx, row)| if row_idx == i { None } else { row.get(i) })
                    .copied()
                    .sum::<u32>()
        });
        let total_matches_f = total_matches as f64;
        let total_opponents = (count.saturating_sub(1)) as f64;
        let confidence = if total_matches >= 1 && total_opponents > 1.0 {
            let variance_component = (0.25 / total_matches_f).sqrt();
            let coverage =
                ((total_opponents - total_matches_f).max(0.0) / (total_opponents - 1.0)).sqrt();
            let interval = 1.96 * variance_component * coverage;
            (1.0 - interval).clamp(0.0, 1.0).powf(2.0)
        } else {
            0.0
        };
        let uncertainty = (1.0 - confidence).max(0.0).max(MIN_WEIGHT);
        first_weights.push((ability_bias * uncertainty).max(MIN_WEIGHT));
    }

    let left_index = sample_index(&first_weights, &mut rng)?;

    // Determine which opponents are still fresh (no games recorded against `left_index`).
    let mut fresh_candidates = Vec::new();
    let mut fallback_candidates = Vec::new();
    for j in 0..count {
        if j == left_index {
            continue;
        }
        let matches = win_matrix[left_index].get(j).copied().unwrap_or(0)
            + win_matrix
                .get(j)
                .and_then(|row| row.get(left_index))
                .copied()
                .unwrap_or(0);
        if matches == 0 {
            fresh_candidates.push(j);
        }
        fallback_candidates.push((j, matches));
    }

    let candidate_source: Vec<usize> = if !fresh_candidates.is_empty() {
        fresh_candidates
    } else {
        fallback_candidates
            .into_iter()
            .map(|(idx, _)| idx)
            .collect()
    };

    if candidate_source.is_empty() {
        return None;
    }

    let mut second_weights = Vec::with_capacity(candidate_source.len());
    for &j in &candidate_source {
        let rating_gap = (abilities[left_index] - abilities[j]).abs();
        let proximity_bias = (-PROXIMITY_ALPHA * rating_gap).exp();
        let matches = win_matrix[left_index].get(j).copied().unwrap_or(0)
            + win_matrix
                .get(j)
                .and_then(|row| row.get(left_index))
                .copied()
                .unwrap_or(0);
        let freshness_bias = 1.0 / (1.0 + matches as f64);
        let ability_bias = abilities[j].max(MIN_WEIGHT);

        let mut weight = ability_bias * proximity_bias * freshness_bias;
        if let Some(previous) = last {
            if (previous.left_index == left_index && previous.right_index == j)
                || (previous.left_index == j && previous.right_index == left_index)
            {
                weight *= RECENT_PAIR_PENALTY;
            }
        }
        second_weights.push(weight.max(MIN_WEIGHT));
    }

    let right_index = if let Some(idx) = sample_index(&second_weights, &mut rng) {
        candidate_source[idx]
    } else {
        let mut remaining = candidate_source;
        remaining.swap_remove(rng.gen_range(0..remaining.len()))
    };

    Some(Matchup {
        left_index,
        right_index,
    })
}

fn sample_index(weights: &[f64], rng: &mut impl Rng) -> Option<usize> {
    if weights.is_empty() {
        return None;
    }
    if weights.iter().all(|w| !w.is_finite() || *w <= 0.0) {
        return None;
    }
    let sanitized: Vec<f64> = weights
        .iter()
        .map(|w| {
            if w.is_finite() && *w > 0.0 {
                *w
            } else {
                MIN_WEIGHT
            }
        })
        .collect();
    WeightedIndex::new(&sanitized)
        .ok()
        .map(|dist| dist.sample(rng))
}
