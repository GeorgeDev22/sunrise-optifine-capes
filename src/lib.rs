//! OptiFine capes for Sunrise: a player who got a cape from OptiFine wears it, as OptiFine and
//! the cape mods for Java show it.
//!
//! - `downloads` asks OptiFine's cape server for a player's cape by name, on a thread of its own;
//! - `capes` keeps the answers, uploads the capes of the players drawn lately as dynamic textures,
//!   and answers the game's `resolve_player_layer` with them;
//! - what doesn't need the game (the name check, the picture's layout, which capes are shown) is
//!   in `optifine-capes-core` (`src/core/`), where it is tested.

mod capes;
mod downloads;
mod settings;

use mod_api::{
    ClientHost, ClientLevelLoadEvent, ClientTickPostEvent, Hooks, Registrar, RegistrationContext,
    ResolvePlayerLayerEvent, SettingChangedEvent,
};

#[derive(Default)]
struct OptifineCapes;

#[mod_api::sunrise_mod]
impl Hooks for OptifineCapes {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        settings::register(registrar);
    }

    /// Settings can be read once a world is open: read them each time one opens.
    fn client_level_load(&self, host: &mut dyn ClientHost, _: &mut ClientLevelLoadEvent<'_>) {
        settings::load(&*host);
        capes::new_world();
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _: &mut SettingChangedEvent<'_>) {
        settings::load(&*host);
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        capes::tick(host);
    }

    fn resolve_player_layer(
        &self,
        _: &dyn ClientHost,
        event: &mut ResolvePlayerLayerEvent<'_, '_>,
    ) {
        capes::choose(event);
    }
}
