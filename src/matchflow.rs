use rand::seq::SliceRandom;

#[derive(Debug, Clone)]
pub struct Matchup {
    pub left_index: usize,
    pub right_index: usize,
}

pub fn random_matchup(count: usize, last: Option<&Matchup>) -> Option<Matchup> {
    if count < 2 {
        return None;
    }

    let mut rng = rand::thread_rng();
    let mut candidates: Vec<usize> = (0..count).collect();

    if let Some(previous) = last {
        candidates.retain(|index| *index != previous.left_index && *index != previous.right_index);
        if candidates.len() < 2 {
            candidates = (0..count).collect();
        }
    }

    candidates.shuffle(&mut rng);
    let left = *candidates.first()?;

    let mut right_candidates: Vec<usize> =
        (0..count).filter(|index| *index != left).collect();

    if let Some(previous) = last {
        right_candidates.retain(|index| {
            *index != previous.left_index && *index != previous.right_index
        });
        if right_candidates.is_empty() {
            right_candidates = (0..count).filter(|index| *index != left).collect();
        }
    }

    right_candidates.shuffle(&mut rng);
    let mut right = *right_candidates.first()?;

    if let Some(previous) = last {
        if (left == previous.left_index && right == previous.right_index)
            || (left == previous.right_index && right == previous.left_index)
        {
            if let Some(new_right) = right_candidates.iter().copied().find(|candidate| {
                (*candidate != previous.left_index && *candidate != previous.right_index)
                    && *candidate != left
            }) {
                right = new_right;
            }
        }
    }

    Some(Matchup {
        left_index: left,
        right_index: right,
    })
}
