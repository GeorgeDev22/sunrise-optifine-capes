//! The mod's page in Options > Mods: one switch, and the copy of it the hooks read.

use mod_api::{ClientHost, Registrar, SettingSpec};
use std::sync::atomic::{AtomicBool, Ordering};

const ENABLED: &str = "enabled";
/// On by default: installing the mod is the choice to see OptiFine capes.
const DEFAULT: bool = true;

static ON: AtomicBool = AtomicBool::new(DEFAULT);

pub fn register(registrar: &mut dyn Registrar) {
    let spec = SettingSpec::toggle(ENABLED, "optifine_capes.setting.enabled", DEFAULT)
        .tooltip("optifine_capes.setting.enabled.tooltip");
    if let Err(error) = registrar.register_setting(spec) {
        eprintln!("optifine_capes: the {ENABLED} setting was refused: {error:?}");
    }
}

/// Reads the player's choice.
pub fn load(host: &dyn ClientHost) {
    let on = host.setting_bool(ENABLED).unwrap_or(DEFAULT);
    ON.store(on, Ordering::Relaxed);
}

/// Whether capes are shown, and so whether the cape server is asked anything.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}
