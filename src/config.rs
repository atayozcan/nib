//! KDL-backed configuration.
//!
//! The compiled-in [`DEFAULT_CONFIG`] is always loaded first; if a user config exists
//! at `$XDG_CONFIG_HOME/nib/nib.kdl`, it is parsed and applied *on top of* the
//! defaults — later definitions override earlier ones. That way an unconfigured launch
//! still does the right thing, and a user config only has to declare what differs.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use etcetera::{AppStrategy, AppStrategyArgs, choose_app_strategy};
use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::keymap::{KeyChord, KeyMap};
use crate::mode::Mode;
use crate::term::Color;

pub(crate) const DEFAULT_CONFIG: &str = include_str!("../assets/nib.default.kdl");

#[derive(Debug, Clone)]
pub(crate) struct Theme {
    pub(crate) foreground: Color,
    pub(crate) background: Color,
    pub(crate) status_fg: Color,
    pub(crate) status_bg: Color,
    pub(crate) cmdline_fg: Color,
    pub(crate) cmdline_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: Color::Reset,
            background: Color::Reset,
            status_fg: Color::Reset,
            status_bg: Color::Reset,
            cmdline_fg: Color::Reset,
            cmdline_bg: Color::Reset,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Behavior {
    pub(crate) tab_width: u16,
    pub(crate) line_numbers: bool,
    pub(crate) chord_timeout_ms: u16,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            tab_width: 4,
            line_numbers: false,
            chord_timeout_ms: 500,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) theme: Theme,
    pub(crate) behavior: Behavior,
    /// One keymap per mode. Modes not present here get an empty (no-op) keymap.
    pub(crate) keymaps: HashMap<Mode, KeyMap>,
}

impl Default for Config {
    fn default() -> Self {
        let mut keymaps = HashMap::new();
        for &m in Mode::ALL {
            keymaps.insert(m, KeyMap::default());
        }
        Self {
            theme: Theme::default(),
            behavior: Behavior::default(),
            keymaps,
        }
    }
}

impl Config {
    /// Load the compiled-in defaults, then layer the user's `nib.kdl` on top if present.
    ///
    /// The defaults *must* parse — a failure here is a build-time bug and is propagated.
    /// A broken *user* config is intentionally soft-failed: the editor starts with
    /// defaults, and the caller is handed a warning string to surface in the UI so the
    /// user actually notices instead of being locked out of their files.
    pub(crate) fn load() -> (Self, Option<String>) {
        let mut cfg = Self::default();
        // Defaults are compiled in; any error here is on us, not the user.
        cfg.apply_kdl(DEFAULT_CONFIG, "nib.default.kdl")
            .expect("built-in default config must parse");

        let Some(path) = user_config_path() else {
            return (cfg, None);
        };
        if !path.exists() {
            return (cfg, None);
        }

        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                return (
                    cfg,
                    Some(format!("config: {} unreadable: {e}", path.display())),
                );
            }
        };

        // Apply on a temporary clone so a parse error halfway through doesn't leave the
        // config in a partially-overridden state.
        let mut staged = cfg.clone();
        match staged.apply_kdl(&text, &path.display().to_string()) {
            Ok(()) => (staged, None),
            Err(e) => (cfg, Some(format!("config: {e:#}"))),
        }
    }

    fn apply_kdl(&mut self, text: &str, source: &str) -> Result<()> {
        let doc: KdlDocument = text.parse().with_context(|| format!("parsing {source}"))?;
        for node in doc.nodes() {
            match node.name().value() {
                "theme" => self.apply_theme(node)?,
                "behavior" => self.apply_behavior(node)?,
                "keymap" => self.apply_keymap(node)?,
                other => bail!("{source}: unknown top-level node {other:?}"),
            }
        }
        Ok(())
    }

    fn apply_theme(&mut self, node: &KdlNode) -> Result<()> {
        let Some(children) = node.children() else {
            return Ok(());
        };
        for child in children.nodes() {
            let value = first_string(child)?;
            let color =
                parse_color(value).with_context(|| format!("theme.{}", child.name().value()))?;
            match child.name().value() {
                "foreground" => self.theme.foreground = color,
                "background" => self.theme.background = color,
                "status_fg" => self.theme.status_fg = color,
                "status_bg" => self.theme.status_bg = color,
                "cmdline_fg" => self.theme.cmdline_fg = color,
                "cmdline_bg" => self.theme.cmdline_bg = color,
                other => bail!("unknown theme key {other:?}"),
            }
        }
        Ok(())
    }

    fn apply_behavior(&mut self, node: &KdlNode) -> Result<()> {
        let Some(children) = node.children() else {
            return Ok(());
        };
        for child in children.nodes() {
            match child.name().value() {
                "tab_width" => {
                    self.behavior.tab_width = first_int(child)?
                        .try_into()
                        .context("tab_width out of range")?;
                }
                "line_numbers" => self.behavior.line_numbers = first_bool(child)?,
                "chord_timeout_ms" => {
                    self.behavior.chord_timeout_ms = first_int(child)?
                        .try_into()
                        .context("chord_timeout_ms out of range")?;
                }
                other => bail!("unknown behavior key {other:?}"),
            }
        }
        Ok(())
    }

    fn apply_keymap(&mut self, node: &KdlNode) -> Result<()> {
        let mode_name = first_string(node)?;
        let mode =
            Mode::from_str(mode_name).with_context(|| format!("in keymap node {mode_name:?}"))?;
        let km = self.keymaps.entry(mode).or_default();
        if let Some(children) = node.children() {
            walk_keymap_block(km, &mut Vec::new(), children)?;
        }
        Ok(())
    }
}

fn walk_keymap_block(
    km: &mut KeyMap,
    prefix: &mut Vec<KeyChord>,
    block: &KdlDocument,
) -> Result<()> {
    for node in block.nodes() {
        let chord = KeyChord::parse(node.name().value())
            .with_context(|| format!("in keymap chord {:?}", node.name().value()))?;
        prefix.push(chord);
        match (node.children(), first_string_opt(node)) {
            (Some(children), _) => {
                walk_keymap_block(km, prefix, children)?;
            }
            (None, Some(cmd)) => km.bind(prefix, cmd),
            (None, None) => bail!("keymap leaf without a command: {:?}", node.name().value()),
        }
        prefix.pop();
    }
    Ok(())
}

fn first_string(node: &KdlNode) -> Result<&str> {
    first_string_opt(node)
        .with_context(|| format!("{} needs a string argument", node.name().value()))
}

fn first_string_opt(node: &KdlNode) -> Option<&str> {
    node.entries()
        .iter()
        .find_map(|e: &KdlEntry| e.value().as_string())
}

fn first_int(node: &KdlNode) -> Result<i128> {
    node.entries()
        .iter()
        .find_map(|e| e.value().as_integer())
        .with_context(|| format!("{} needs an integer", node.name().value()))
}

fn first_bool(node: &KdlNode) -> Result<bool> {
    node.entries()
        .iter()
        .find_map(|e| e.value().as_bool())
        .with_context(|| format!("{} needs a boolean", node.name().value()))
}

fn parse_color(s: &str) -> Result<Color> {
    if let Some(rest) = s.strip_prefix('#') {
        if rest.len() != 6 {
            bail!("hex color must be #RRGGBB, got {s:?}");
        }
        let r = u8::from_str_radix(&rest[0..2], 16).context("R")?;
        let g = u8::from_str_radix(&rest[2..4], 16).context("G")?;
        let b = u8::from_str_radix(&rest[4..6], 16).context("B")?;
        return Ok(Color::Rgb(r, g, b));
    }
    if let Some(rest) = s.strip_prefix("idx:") {
        let n: u8 = rest.parse().context("indexed color must be 0..=255")?;
        return Ok(Color::Indexed(n));
    }
    if s == "reset" {
        return Ok(Color::Reset);
    }
    bail!("unknown color {s:?}; expected #RRGGBB, idx:<n>, or 'reset'");
}

fn user_config_path() -> Option<PathBuf> {
    let args = AppStrategyArgs {
        top_level_domain: "io.github".into(),
        author: "atayozcan".into(),
        app_name: "nib".into(),
    };
    let strategy = choose_app_strategy(args).ok()?;
    Some(strategy.config_dir().join("nib.kdl"))
}
