//! What the cape server has been asked this session, and its answers: each name is asked once,
//! and a cape and a miss alike are kept.

use crate::picture::{Cape, valid_name};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// How many questions are out at once.
pub const MAX_ASKING: usize = 4;
/// How many answers are kept. Past this the oldest settled one goes, and is asked again if its
/// player comes back: a session on a busy server doesn't grow without end.
pub const MAX_ANSWERS: usize = 1024;

/// Where one name stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Asking,
    /// The server has no cape for the name, or answered something that isn't one.
    NoCape,
    Cape(Arc<Cape>),
}

/// The session's questions and answers, by name ignoring ASCII case: Minecraft names are.
#[derive(Debug, Default)]
pub struct Lookups {
    answers: HashMap<String, Answer>,
    /// The `answers` keys, oldest first.
    order: VecDeque<String>,
    asking: usize,
}

impl Lookups {
    /// The names of `players` to ask about now: valid ([`valid_name`]), not asked yet this
    /// session, and no more than [`MAX_ASKING`] out at once. They are marked as being asked;
    /// each is given as the player list spells it.
    pub fn to_ask<'a>(&mut self, players: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut ask = Vec::new();
        for name in players {
            if self.asking >= MAX_ASKING {
                break;
            }
            let key = name.to_ascii_lowercase();
            if !valid_name(name) || self.answers.contains_key(&key) {
                continue;
            }
            self.keep(key, Answer::Asking);
            self.asking += 1;
            ask.push(name.to_owned());
        }
        ask
    }

    /// The server's answer for `name`: its cape, or `None` for no cape.
    pub fn answered(&mut self, name: &str, cape: Option<Cape>) {
        self.asking = self.asking.saturating_sub(1);
        let answer = match cape {
            Some(cape) => Answer::Cape(Arc::new(cape)),
            None => Answer::NoCape,
        };
        self.keep(name.to_ascii_lowercase(), answer);
    }

    /// `name`'s cape, once the server answered with one.
    pub fn cape(&self, name: &str) -> Option<&Arc<Cape>> {
        match self.answers.get(&name.to_ascii_lowercase()) {
            Some(Answer::Cape(cape)) => Some(cape),
            _ => None,
        }
    }

    /// Where `name` stands, if it was asked.
    pub fn answer(&self, name: &str) -> Option<&Answer> {
        self.answers.get(&name.to_ascii_lowercase())
    }

    fn keep(&mut self, key: String, answer: Answer) {
        if self.answers.insert(key.clone(), answer).is_some() {
            return;
        }
        self.order.push_back(key);
        while self.answers.len() > MAX_ANSWERS {
            let Some(at) = self
                .order
                .iter()
                .position(|k| self.answers.get(k) != Some(&Answer::Asking))
            else {
                break;
            };
            if let Some(old) = self.order.remove(at) {
                self.answers.remove(&old);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cape() -> Cape {
        Cape {
            width: 64,
            height: 32,
            rgba: vec![1; 64 * 32 * 4],
            elytra: true,
        }
    }

    #[test]
    fn each_valid_name_is_asked_once_and_its_answer_kept() {
        let mut lookups = Lookups::default();
        let asked = lookups.to_ask(["Steve", "not a name", "", "../x", "Alex"]);
        assert_eq!(asked, ["Steve", "Alex"]);
        // The same names again, in another case too: nothing new to ask while they are out.
        assert!(lookups.to_ask(["Steve", "STEVE", "alex"]).is_empty());
        lookups.answered("Steve", Some(cape()));
        lookups.answered("Alex", None);
        assert!(
            lookups.to_ask(["Steve", "alex"]).is_empty(),
            "answers are kept"
        );
        assert_eq!(lookups.cape("STEVE").map(|c| c.width), Some(64));
        assert_eq!(lookups.cape("Alex"), None);
        assert_eq!(lookups.answer("alex"), Some(&Answer::NoCape));
        assert_eq!(lookups.answer("Notch"), None);
    }

    #[test]
    fn only_a_few_names_are_asked_at_once() {
        let mut lookups = Lookups::default();
        let names = ["a", "b", "c", "d", "e", "f"];
        assert_eq!(lookups.to_ask(names), ["a", "b", "c", "d"]);
        assert!(lookups.to_ask(names).is_empty());
        lookups.answered("b", None);
        assert_eq!(lookups.to_ask(names), ["e"]);
    }

    #[test]
    fn the_oldest_settled_answer_goes_past_the_bound() {
        let mut lookups = Lookups::default();
        for i in 0..MAX_ANSWERS {
            let name = format!("p{i}");
            assert_eq!(lookups.to_ask([name.as_str()]), [name.clone()]);
            lookups.answered(&name, None);
        }
        // One still being asked is never dropped.
        assert_eq!(lookups.to_ask(["late"]), ["late"]);
        assert_eq!(lookups.answer("p0"), None, "the oldest went");
        assert_eq!(lookups.answer("p1"), Some(&Answer::NoCape));
        assert_eq!(lookups.answer("late"), Some(&Answer::Asking));
    }
}
