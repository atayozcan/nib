//! Keymap: from a stream of [`Key`]+[`KeyMod`] events to a named command.
//!
//! Each mode owns an independent trie. A leaf is `Action::Command(name)`; an interior
//! node is `Action::Submap(children)`. The trie supports chord sequences like `gg`,
//! `ge`, `<Space>ff` without any special-casing.
//!
//! The string format used by `KeyChord::parse` mirrors what end users will type into
//! their config: `h`, `<Esc>`, `<C-x>`, `<C-S-Tab>`, `<Space>`, `<F5>`.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};

use crate::term::{Key, KeyMod};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct KeyChord {
    pub(crate) key: Key,
    pub(crate) mods: KeyMod,
}

impl KeyChord {
    pub(crate) fn from_event(key: Key, mods: KeyMod) -> Self {
        Self { key, mods }
    }

    /// Parse a single chord like `<C-x>` / `h` / `<Esc>` from a textual config form.
    pub(crate) fn parse(s: &str) -> Result<Self> {
        // Bare single character → that character with no modifier.
        if s.chars().count() == 1 && !s.starts_with('<') {
            return Ok(Self {
                key: Key::Char(s.chars().next().unwrap()),
                mods: KeyMod::empty(),
            });
        }

        // Otherwise expect `<...>`.
        let inner = s
            .strip_prefix('<')
            .and_then(|s| s.strip_suffix('>'))
            .with_context(|| format!("invalid key chord: {s:?}"))?;

        let mut mods = KeyMod::empty();
        let mut last = None;
        for part in inner.split('-') {
            match part {
                "C" => mods |= KeyMod::CTRL,
                "S" => mods |= KeyMod::SHIFT,
                "A" | "M" => mods |= KeyMod::ALT,
                other => {
                    if last.is_some() {
                        bail!("invalid key chord {s:?}: multiple keys after modifiers");
                    }
                    last = Some(other.to_string());
                }
            }
        }
        let name = last.with_context(|| format!("key chord {s:?} has no terminal key"))?;
        let key = match name.as_str() {
            "Esc" | "Escape" => Key::Escape,
            "Enter" | "CR" | "Return" => Key::Enter,
            "Tab" => Key::Tab,
            "BackTab" => Key::BackTab,
            "BS" | "Backspace" => Key::Backspace,
            "Del" | "Delete" => Key::Delete,
            "Up" => Key::Up,
            "Down" => Key::Down,
            "Left" => Key::Left,
            "Right" => Key::Right,
            "Home" => Key::Home,
            "End" => Key::End,
            "PageUp" | "PgUp" => Key::PageUp,
            "PageDown" | "PgDn" => Key::PageDown,
            "Space" => Key::Char(' '),
            "Insert" => Key::Insert,
            s if s.starts_with('F') => {
                let n: u8 = s[1..].parse().with_context(|| format!("bad F-key: {s}"))?;
                Key::Function(n)
            }
            s if s.chars().count() == 1 => Key::Char(s.chars().next().unwrap()),
            other => bail!("unknown key name in chord: {other:?}"),
        };
        Ok(Self { key, mods })
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct KeyMap {
    root: TrieNode,
}

#[derive(Debug, Default, Clone)]
struct TrieNode {
    children: HashMap<KeyChord, TrieNode>,
    command: Option<String>,
}

#[derive(Debug)]
pub(crate) enum Lookup<'a> {
    /// Chord sequence resolves to a command name.
    Command(&'a str),
    /// Chord is a partial prefix; caller should keep accumulating.
    Pending,
    /// Chord doesn't match any binding from this point.
    None,
}

impl KeyMap {
    pub(crate) fn bind(&mut self, chords: &[KeyChord], command: &str) {
        let mut node = &mut self.root;
        for chord in chords {
            node = node.children.entry(*chord).or_default();
        }
        // Last writer wins — config can override built-in defaults explicitly.
        node.command = Some(command.to_string());
    }

    /// Look up a partial chord sequence. The slice represents what's been typed so far
    /// since the last command resolution.
    pub(crate) fn lookup(&self, chords: &[KeyChord]) -> Lookup<'_> {
        let mut node = &self.root;
        for c in chords {
            match node.children.get(c) {
                Some(n) => node = n,
                None => return Lookup::None,
            }
        }
        if let Some(cmd) = node.command.as_deref() {
            if node.children.is_empty() {
                return Lookup::Command(cmd);
            }
            // Has further chords beyond it — we still resolve, since waiting longer
            // adds no information unless the user types another key. Editors handle
            // this with a small timeout; for v3 we resolve eagerly and let users
            // disambiguate with explicit longer chords.
            return Lookup::Command(cmd);
        }
        if node.children.is_empty() {
            Lookup::None
        } else {
            Lookup::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_char() {
        let k = KeyChord::parse("h").unwrap();
        assert_eq!(k.key, Key::Char('h'));
        assert!(k.mods.is_empty());
    }

    #[test]
    fn parse_ctrl_chord() {
        let k = KeyChord::parse("<C-x>").unwrap();
        assert_eq!(k.key, Key::Char('x'));
        assert_eq!(k.mods, KeyMod::CTRL);
    }

    #[test]
    fn parse_named_key() {
        assert_eq!(KeyChord::parse("<Esc>").unwrap().key, Key::Escape);
        assert_eq!(KeyChord::parse("<F5>").unwrap().key, Key::Function(5));
        assert_eq!(KeyChord::parse("<Space>").unwrap().key, Key::Char(' '));
    }

    #[test]
    fn trie_lookup_chord_sequence() {
        let mut km = KeyMap::default();
        let g = KeyChord::parse("g").unwrap();
        let e = KeyChord::parse("e").unwrap();
        km.bind(&[g, g], "buffer.goto_start");
        km.bind(&[g, e], "buffer.goto_end");

        assert!(matches!(km.lookup(&[g]), Lookup::Pending));
        assert!(matches!(
            km.lookup(&[g, g]),
            Lookup::Command("buffer.goto_start")
        ));
        assert!(matches!(
            km.lookup(&[g, e]),
            Lookup::Command("buffer.goto_end")
        ));
        assert!(matches!(km.lookup(&[e]), Lookup::None));
    }
}
