//! The OptiFine capes mod's logic that needs no game: what may be asked of OptiFine's cape
//! server ([`picture::valid_name`]), which answers are capes and how a cape's picture sits on
//! the cape texture ([`picture`]), what has been asked this session ([`lookups`]), and which
//! players' capes hold the mod's few dynamic textures ([`shown`]).
//!
//! Written from what cape makers and players say of the format (a 46x22 picture on the 64x32
//! cape layout, a 92x44 HD one on 128x64); no OptiFine code was read.

pub mod lookups;
pub mod picture;
pub mod shown;

/// A built-in player layer, as the game's `PlayerLayer` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Cape,
    Wings,
}

/// Whether a player's OptiFine cape is drawn on `layer`: always on the cape, and on worn wings
/// when its picture reaches the elytra's part of the layout (`elytra`,
/// [`picture::Cape::elytra`]). On wings it otherwise leaves the game's own choice alone.
pub fn wears(layer: Layer, elytra: bool) -> bool {
    match layer {
        Layer::Cape => true,
        Layer::Wings => elytra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cape_dresses_wings_only_when_it_reaches_the_elytras_part() {
        assert!(wears(Layer::Cape, true));
        assert!(wears(Layer::Cape, false));
        assert!(wears(Layer::Wings, true));
        assert!(!wears(Layer::Wings, false));
    }
}
