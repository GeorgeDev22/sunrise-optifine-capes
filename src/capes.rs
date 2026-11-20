//! The session's capes: who is asked about, which capes hold the mod's dynamic textures, and the
//! answer to the game's `resolve_player_layer`.
//!
//! The game asks about a player's cape and wings on the render thread, every draw, with a
//! read-only host: [`choose`] only reads what [`tick`] published, without allocating or waiting.
//! [`tick`] does the rest once a client tick: reads the player list, starts questions, takes
//! answers, and uploads or frees textures.

use crate::{downloads, settings};
use mod_api::{ClientHost, Identifier, PlayerLayer, PlayerLayerChoice, ResolvePlayerLayerEvent};
use optifine_capes_core::lookups::Lookups;
use optifine_capes_core::shown::{self, Recent};
use optifine_capes_core::{Layer, wears};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, PoisonError, RwLock};

/// How often the player list is read, in ticks.
const LIST_EVERY: u64 = 20;
/// How often the shown capes are chosen again, in ticks.
const CHOOSE_EVERY: u64 = 5;
/// A player not drawn for this many ticks gives up its texture to one who is.
const DRAWN_WITHIN: u64 = 100;

/// The client ticks so far, the clock the draws are remembered by.
static TICK: AtomicU64 = AtomicU64::new(0);
/// Who was drawn lately, written by the draws.
static DRAWN: Mutex<Recent> = Mutex::new(Recent::new());
/// The window's own player, as the draws name it.
static LOCAL: Mutex<Option<u128>> = Mutex::new(None);
/// The shown capes, by player: the texture and whether it dresses an elytra. What the draws read.
static SHOWN: LazyLock<RwLock<HashMap<u128, (Identifier, bool)>>> = LazyLock::new(Default::default);
/// The rest, only touched by the tick.
static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);

#[derive(Default)]
struct State {
    lookups: Lookups,
    /// Each listed player's profile name, from the last read of the player list.
    names: HashMap<u128, String>,
    /// The textures this mod holds: player, the name its cape is of, and the texture.
    textures: HashMap<u128, (String, Identifier)>,
    /// The player list could be read: a world is open, where textures can be made.
    in_world: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A new world: its players are drawn afresh. The answers are kept for the session.
pub fn new_world() {
    lock(&DRAWN).clear();
    *lock(&LOCAL) = None;
}

/// `resolve_player_layer`: this player's OptiFine cape, if it holds a texture, on their cape
/// and, when the picture reaches the elytra's part, on worn wings. Anything else is left to the
/// game. Remembers that the player was drawn, so their cape is loaded next.
pub fn choose(event: &mut ResolvePlayerLayerEvent<'_, '_>) {
    if !settings::on() {
        return;
    }
    let player = event.player.0;
    // Never wait on the tick: a draw that finds it busy is remembered next time.
    if let Ok(mut drawn) = DRAWN.try_lock() {
        drawn.saw(player, TICK.load(Ordering::Relaxed));
    }
    if event.local
        && let Ok(mut local) = LOCAL.try_lock()
    {
        *local = Some(player);
    }
    let layer = match event.layer {
        PlayerLayer::Cape => Layer::Cape,
        PlayerLayer::Wings => Layer::Wings,
        _ => return,
    };
    let Ok(shown) = SHOWN.try_read() else {
        return;
    };
    if let Some((texture, elytra)) = shown.get(&player)
        && wears(layer, *elytra)
    {
        // A refusal (the texture just went) leaves the game's own choice.
        let _ = event.choose(PlayerLayerChoice::UseTexture(texture));
    }
}

/// Once a client tick: reads the player list, asks about new names, takes the answers, and
/// gives the textures to the capes drawn lately.
pub fn tick(host: &mut dyn ClientHost) {
    let tick = TICK.fetch_add(1, Ordering::Relaxed) + 1;
    let mut state = lock(&STATE);
    for (name, cape) in downloads::answers() {
        state.lookups.answered(&name, cape);
    }
    if !settings::on() {
        // Off: nothing is asked, and the textures go.
        state.show(host, &[]);
        return;
    }
    if tick % LIST_EVERY == 0 {
        let list = host.client_session().client_tab_list();
        state.in_world = list.is_some();
        if let Some(list) = list {
            state.names = list
                .entries
                .iter()
                .map(|entry| (entry.uuid, entry.name.clone()))
                .collect();
            let names = list.entries.iter().map(|entry| entry.name.as_str());
            for name in state.lookups.to_ask(names) {
                downloads::ask(name);
            }
        }
    }
    if tick % CHOOSE_EVERY == 0 {
        let recent = lock(&DRAWN).clone();
        let local = *lock(&LOCAL);
        let since = tick.saturating_sub(DRAWN_WITHIN);
        let wanted = shown::choose(local, &recent, since, |player| {
            state.cape_name(player).is_some()
        });
        state.show(host, &wanted);
    }
}

impl State {
    /// The name of `player`'s cape, when the server answered with one.
    fn cape_name(&self, player: u128) -> Option<&str> {
        let name = self.names.get(&player)?;
        self.lookups.cape(name).map(|_| name.as_str())
    }

    /// Gives the textures to `wanted`'s capes: frees those of players no longer wanted (or whose
    /// name changed), makes those missing, and publishes the result for the draws.
    fn show(&mut self, host: &mut dyn ClientHost, wanted: &[u128]) {
        if !self.in_world {
            return;
        }
        let stale: Vec<u128> = self
            .textures
            .iter()
            .filter(|(player, (name, _))| {
                !wanted.contains(player) || self.cape_name(**player) != Some(name.as_str())
            })
            .map(|(player, _)| *player)
            .collect();
        let changed = !stale.is_empty();
        for player in stale {
            if let Some((_, texture)) = self.textures.remove(&player) {
                let _ = host.free_dynamic_texture(&texture);
            }
        }
        let mut added = false;
        for &player in wanted {
            if self.textures.contains_key(&player) {
                continue;
            }
            let Some(name) = self.cape_name(player).map(str::to_owned) else {
                continue;
            };
            let Some(cape) = self.lookups.cape(&name).cloned() else {
                continue;
            };
            let Ok(texture) =
                Identifier::from_parts("optifine_capes", &format!("cape/{player:032x}"))
            else {
                continue;
            };
            let made = host
                .create_dynamic_texture(&texture, cape.width, cape.height, &cape.rgba)
                // Still held from before (a world left while it was shown): take its slot again.
                .or_else(|_| {
                    host.update_dynamic_texture(&texture, cape.width, cape.height, &cape.rgba)
                });
            if made.is_ok() {
                self.textures.insert(player, (name, texture));
                added = true;
            }
        }
        if changed || added {
            let shown = self
                .textures
                .iter()
                .map(|(player, (name, texture))| {
                    let elytra = self.lookups.cape(name).is_some_and(|cape| cape.elytra);
                    (*player, (texture.clone(), elytra))
                })
                .collect();
            *SHOWN.write().unwrap_or_else(PoisonError::into_inner) = shown;
        }
    }
}
