use rand::seq::SliceRandom;

#[derive(Debug, Clone)]
pub struct Matchup {
    pub left_id: String,
    pub right_id: String,
}

pub fn random_matchup(items: &[String], last_match: Option<&Matchup>) -> Option<Matchup> {
    if items.len() < 2 {
        return None;
    }

    let mut rng = rand::thread_rng();

    let mut candidates: Vec<&String> = items.iter().collect();

    if let Some(previous) = last_match {
        candidates.retain(|id| *id != &previous.left_id && *id != &previous.right_id);

        if candidates.len() < 2 {
            candidates = items.iter().collect();
        }
    }

    let left = *candidates.choose(&mut rng)?;
    let mut right = *candidates.choose(&mut rng)?;

    while right == left {
        right = *candidates.choose(&mut rng)?;
    }

    Some(Matchup {
        left_id: left.clone(),
        right_id: right.clone(),
    })
}
