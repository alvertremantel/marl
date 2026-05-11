use crate::cell::Ruleset;
use rand::Rng;

/// Transfer a random reaction rule from donor to recipient.
/// This transfers complete reaction rules (not individual parameters),
/// paralleling real bacterial HGT of metabolic operons.
pub fn transfer_reaction(donor: &Ruleset, recipient: &mut Ruleset, rng: &mut impl Rng) {
    // Find a non-trivial reaction in the donor
    let active: Vec<usize> = (0..donor.reactions.len())
        .filter(|&i| donor.reactions[i].v_max.abs() > 1e-9)
        .collect();

    if active.is_empty() {
        return;
    }

    // Pick a random active reaction from donor
    let donor_idx = active[rng.random_range(0..active.len())];
    let donated = donor.reactions[donor_idx].clone();

    // Find an inactive slot in recipient (or overwrite random slot)
    let target_idx = (0..recipient.reactions.len())
        .find(|&i| recipient.reactions[i].v_max.abs() < 1e-9)
        .unwrap_or_else(|| rng.random_range(0..recipient.reactions.len()));

    recipient.reactions[target_idx] = donated;
}
