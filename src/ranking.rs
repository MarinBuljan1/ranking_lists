use std::collections::HashMap;

const DEFAULT_RATING: f64 = 0.0;
pub const DEFAULT_K_FACTOR: f64 = 0.8;
const MAX_STEP: f64 = 5.0;
const MIN_RATING: f64 = -10.0;
const MAX_RATING: f64 = 10.0;

#[derive(Debug, Clone)]
pub struct BradleyTerry {
    ratings: HashMap<String, f64>,
    k_factor: f64,
}

impl Default for BradleyTerry {
    fn default() -> Self {
        Self::new(DEFAULT_K_FACTOR)
    }
}

impl BradleyTerry {
    pub fn new(k_factor: f64) -> Self {
        Self {
            ratings: HashMap::new(),
            k_factor,
        }
    }

    pub fn from_ratings(ratings: HashMap<String, f64>, k_factor: f64) -> Self {
        Self { ratings, k_factor }
    }

    pub fn ratings(&self) -> &HashMap<String, f64> {
        &self.ratings
    }

    pub fn rating(&self, id: &str) -> f64 {
        self.ratings.get(id).copied().unwrap_or(DEFAULT_RATING)
    }

    pub fn expected_score(&self, a: &str, b: &str) -> f64 {
        let rating_a = self.rating(a);
        let rating_b = self.rating(b);
        let diff = (rating_a - rating_b).clamp(-MAX_STEP, MAX_STEP);
        1.0 / (1.0 + (-diff).exp())
    }

    pub fn update(&mut self, winner: &str, loser: &str) {
        if winner == loser {
            return;
        }

        let expected_winner = self.expected_score(winner, loser);
        let expected_loser = 1.0 - expected_winner;

        let winner_delta = self.k_factor * (1.0 - expected_winner);
        let loser_delta = self.k_factor * (0.0 - expected_loser);

        self.apply_delta(winner, winner_delta);
        self.apply_delta(loser, loser_delta);
    }

    pub fn leaderboard(&self) -> Vec<(String, f64)> {
        let mut entries: Vec<_> = self
            .ratings
            .iter()
            .map(|(id, rating)| (id.clone(), *rating))
            .collect();

        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        entries
    }

    fn apply_delta(&mut self, id: &str, delta: f64) {
        let current = self.rating(id);
        let updated = (current + delta).clamp(MIN_RATING, MAX_RATING);
        self.ratings.insert(id.to_string(), updated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_score_balanced() {
        let system = BradleyTerry::default();
        let expected = system.expected_score("apple", "banana");
        assert!((expected - 0.5).abs() < 1e-9);
    }

    #[test]
    fn ratings_update_direction() {
        let mut system = BradleyTerry::default();
        system.update("apple", "banana");
        assert!(system.rating("apple") > 0.0);
        assert!(system.rating("banana") < 0.0);
    }

    #[test]
    fn leaderboard_sorted() {
        let mut system = BradleyTerry::default();
        system.update("apple", "banana");
        system.update("apple", "banana");
        system.update("banana", "cherry");

        let leaderboard = system.leaderboard();
        assert!(leaderboard
            .windows(2)
            .all(|pair| pair[0].1 >= pair[1].1));
        assert!(leaderboard.iter().any(|(id, _)| id == "apple"));
    }

    #[test]
    fn repeated_updates_respect_clamp() {
        let mut system = BradleyTerry::default();
        for _ in 0..1000 {
            system.update("apple", "banana");
        }
        assert!(system.rating("apple") <= MAX_RATING);
        assert!(system.rating("banana") >= MIN_RATING);
    }
}
