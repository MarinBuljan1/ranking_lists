const MIN_ABILITY: f64 = 1e-6;
const DISPLAY_BASE: f64 = 1000.0;
const DISPLAY_SCALE: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct BradleyTerry {
    abilities: Vec<f64>,
}

impl BradleyTerry {
    pub fn new(count: usize) -> Self {
        Self {
            abilities: vec![1.0; count],
        }
    }

    pub fn from_abilities(abilities: Vec<f64>) -> Self {
        if abilities.is_empty() {
            Self::new(0)
        } else {
            Self {
                abilities: abilities
                    .into_iter()
                    .map(|value| value.max(MIN_ABILITY))
                    .collect(),
            }
        }
    }

    pub fn abilities(&self) -> &[f64] {
        &self.abilities
    }

    pub fn abilities_mut(&mut self) -> &mut [f64] {
        &mut self.abilities
    }

    pub fn ensure_len(&mut self, len: usize) {
        if self.abilities.len() < len {
            self.abilities
                .resize(len, if len > 0 { 1.0 / len as f64 } else { 1.0 });
        } else if self.abilities.len() > len {
            self.abilities.truncate(len);
        }
        if !self.abilities.is_empty() {
            self.normalize();
        }
    }

    pub fn expected_score(&self, i: usize, j: usize) -> f64 {
        if self.abilities.is_empty() {
            return 0.5;
        }
        let ai = self.abilities[i].max(MIN_ABILITY);
        let aj = self.abilities[j].max(MIN_ABILITY);
        ai / (ai + aj)
    }

    pub fn run_iterations(&mut self, wins: &[Vec<u32>], iterations: usize) {
        let n = wins.len();
        if n == 0 || iterations == 0 {
            return;
        }
        self.ensure_len(n);

        let mut abilities = self.abilities.clone();

        for _ in 0..iterations {
            let mut updated = abilities.clone();
            for i in 0..n {
                let wins_i: f64 = wins[i].iter().map(|&w| w as f64).sum();
                if wins_i <= f64::EPSILON {
                    continue;
                }

                let mut denom = 0.0;
                for j in 0..n {
                    if i == j {
                        continue;
                    }
                    let total = wins[i][j] + wins[j][i];
                    if total == 0 {
                        continue;
                    }
                    denom += (total as f64) / (abilities[i] + abilities[j] + MIN_ABILITY);
                }

                if denom > 0.0 {
                    updated[i] = (wins_i / denom).max(MIN_ABILITY);
                }
            }

            normalize(&mut updated);
            abilities = updated;
        }

        self.abilities = abilities;
    }

    pub fn log_score(&self, index: usize) -> f64 {
        self.abilities
            .get(index)
            .copied()
            .unwrap_or(1.0)
            .ln()
    }

    pub fn display_rating(&self, index: usize) -> f64 {
        let ability = self
            .abilities
            .get(index)
            .copied()
            .unwrap_or(1.0)
            .max(MIN_ABILITY);
        let count = self.abilities.len().max(1) as f64;
        let adjusted = ability * count;
        (DISPLAY_BASE + DISPLAY_SCALE * adjusted.ln()).max(0.0)
    }

    pub fn to_vec(&self) -> Vec<f64> {
        self.abilities.clone()
    }

    fn normalize(&mut self) {
        normalize(&mut self.abilities);
    }
}

fn normalize(values: &mut [f64]) {
    let sum: f64 = values.iter().map(|v| v.max(MIN_ABILITY)).sum();
    if sum <= f64::EPSILON {
        let len = values.len();
        if len == 0 {
            return;
        }
        let uniform = 1.0 / len as f64;
        for value in values.iter_mut() {
            *value = uniform;
        }
    } else {
        for value in values.iter_mut() {
            *value = value.max(MIN_ABILITY) / sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_keeps_sum_one() {
        let mut system = BradleyTerry::new(3);
        system.run_iterations(&vec![vec![0, 5, 0], vec![0, 0, 0], vec![0, 0, 0]], 5);
        let sum: f64 = system.abilities().iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn abilities_increase_for_winner() {
        let wins = vec![
            vec![0, 3, 0],
            vec![0, 0, 0],
            vec![0, 0, 0],
        ];

        let mut system = BradleyTerry::new(3);
        system.run_iterations(&wins, 10);

        assert!(system.abilities()[0] > system.abilities()[1]);
        assert!(system.abilities()[0] > system.abilities()[2]);
    }
}
