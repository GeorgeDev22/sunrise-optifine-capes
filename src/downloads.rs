//! Asking OptiFine's cape server for a player's cape, each question on a thread of its own so the
//! game never waits for the network. Nothing is written to disk.

use optifine_capes_core::picture::{Cape, MAX_BYTES, valid_name};
use std::io::Read;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};
use std::time::Duration;

/// OptiFine's cape server: a player's cape is `<this><name>.png`, a 404 when they have none.
/// Plain HTTP, as OptiFine itself asks it: the server answers no HTTPS.
const SERVER: &str = "http://s.optifine.net/capes/";

type Answer = (String, Option<Cape>);

static ANSWERS: LazyLock<(Sender<Answer>, Mutex<Receiver<Answer>>)> = LazyLock::new(|| {
    let (send, answers) = channel();
    (send, Mutex::new(answers))
});

/// Where the capes are asked for: OptiFine's server, or the one `SUNRISE_OPTIFINE_CAPES` names
/// (a test's own), read once.
fn server() -> &'static str {
    static OVERRIDE: OnceLock<String> = OnceLock::new();
    OVERRIDE.get_or_init(|| {
        std::env::var("SUNRISE_OPTIFINE_CAPES")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| SERVER.to_owned())
    })
}

/// Asks the server for `name`'s cape. Its answer comes back through [`answers`], a cape or
/// `None` for anything else (a 404, an error, something that isn't a cape), always exactly once.
pub fn ask(name: String) {
    let send = ANSWERS.0.clone();
    // The core's name check again, here where the address is built: nothing else goes in it.
    if !valid_name(&name) {
        let _ = send.send((name, None));
        return;
    }
    let url = format!("{}{name}.png", server());
    let thread = std::thread::Builder::new().name("OptiFine cape".into());
    let reply = send.clone();
    let asked = name.clone();
    let started = thread.spawn(move || {
        let cape = std::panic::catch_unwind(|| fetch(&url).and_then(|body| Cape::from_png(&body)))
            .ok()
            .flatten();
        let _ = reply.send((asked, cape));
    });
    if started.is_err() {
        let _ = send.send((name, None));
    }
}

/// The answers that came since the last call.
pub fn answers() -> Vec<Answer> {
    ANSWERS
        .1
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .try_iter()
        .collect()
}

/// The body of a 200 answer, read up to one byte past the size a cape may have so a longer one
/// is refused without reading it all. `None` for any other status or an error.
fn fetch(url: &str) -> Option<Vec<u8>> {
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .redirects(0)
        .build()
        .get(url)
        .call()
        .ok()?;
    if response.status() != 200 {
        return None;
    }
    let mut body = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .ok()?;
    Some(body)
}
