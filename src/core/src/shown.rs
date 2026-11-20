//! Which players' capes hold the mod's dynamic textures. A mod holds at most sixteen
//! (`DYNAMIC_TEXTURE_MOD_IMAGES`), so the capes drawn lately get them: your own first.

/// The most capes shown at once: a mod's dynamic textures.
pub const MAX_SHOWN: usize = 16;
/// How many players the draws remember.
pub const RECENT: usize = 64;

/// The players whose cape or wings the game drew lately, and when, kept without allocating: the
/// draw hook writes here for every player it is asked about.
#[derive(Clone, Debug)]
pub struct Recent {
    /// `(uuid, tick)`, a UUID of 0 for an empty place.
    seen: [(u128, u64); RECENT],
}

impl Default for Recent {
    fn default() -> Self {
        Self::new()
    }
}

impl Recent {
    pub const fn new() -> Self {
        Self {
            seen: [(0, 0); RECENT],
        }
    }

    /// `player` was drawn at `tick`: it takes its own place, else an empty one, else the
    /// longest-undrawn player's.
    pub fn saw(&mut self, player: u128, tick: u64) {
        let at = self
            .seen
            .iter()
            .position(|&(uuid, _)| uuid == player)
            .or_else(|| self.seen.iter().position(|&(uuid, _)| uuid == 0))
            .unwrap_or_else(|| {
                (0..RECENT)
                    .min_by_key(|&i| self.seen[i].1)
                    .unwrap_or_default()
            });
        self.seen[at] = (player, tick);
    }

    /// Forgets every player: a new world.
    pub fn clear(&mut self) {
        self.seen = [(0, 0); RECENT];
    }
}

/// The players whose capes take the textures: `local` (the window's own player) first if it
/// has a cape, then those drawn since `since`, the latest first, that have one; at most
/// [`MAX_SHOWN`].
pub fn choose(
    local: Option<u128>,
    recent: &Recent,
    since: u64,
    has_cape: impl Fn(u128) -> bool,
) -> Vec<u128> {
    let mut drawn: Vec<(u128, u64)> = recent
        .seen
        .iter()
        .copied()
        .filter(|&(uuid, tick)| uuid != 0 && tick >= since && Some(uuid) != local)
        .collect();
    drawn.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    local
        .into_iter()
        .chain(drawn.into_iter().map(|(uuid, _)| uuid))
        .filter(|&uuid| has_cape(uuid))
        .take(MAX_SHOWN)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn your_own_cape_first_then_the_latest_drawn() {
        let mut recent = Recent::new();
        recent.saw(7, 10);
        recent.saw(8, 12);
        recent.saw(9, 11);
        recent.saw(42, 12);
        // 9 has no cape; 7 was drawn too long ago.
        let chosen = choose(Some(42), &recent, 11, |uuid| uuid != 9);
        assert_eq!(chosen, [42, 8]);
        // Without a cape of its own the local player takes no texture.
        assert_eq!(choose(Some(1), &recent, 0, |uuid| uuid == 7), [7]);
        assert_eq!(choose(None, &recent, 13, |_| true), Vec::<u128>::new());
    }

    #[test]
    fn at_most_sixteen_capes_are_shown() {
        let mut recent = Recent::new();
        for player in 1..=40 {
            recent.saw(player, player as u64);
        }
        let chosen = choose(Some(1), &recent, 0, |_| true);
        assert_eq!(chosen.len(), MAX_SHOWN);
        assert_eq!(chosen[0], 1);
        assert_eq!(chosen[1..], (26..=40).rev().collect::<Vec<u128>>()[..]);
    }

    #[test]
    fn a_full_memory_forgets_the_longest_undrawn_player() {
        let mut recent = Recent::new();
        for player in 1..=RECENT as u128 {
            recent.saw(player, player as u64);
        }
        recent.saw(1, 100);
        recent.saw(1000, 101);
        let all = choose(None, &recent, 0, |_| true);
        assert!(all.contains(&1000) && all.contains(&1));
        let kept = choose(None, &recent, 0, |uuid| uuid == 2);
        assert!(
            kept.is_empty(),
            "player 2 was the longest undrawn, and went"
        );
    }
}
