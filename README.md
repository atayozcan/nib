# nib

> A minimal, highly-configurable modal terminal text editor for Linux x86_64.

`nib` is small like `vi`, configurable like `kak`, and picks exactly two fights:

- **Pure vi-style modal editing** — `hjkl`, `i a o I A`, `:w :q :wq`, `ZZ`. No `Ctrl-*` bindings.
- **KDL config** to remap, retheme, and redefine chord trees without touching the source.

Out of scope for v0.3.0 (deliberately): cross-platform support, syntax highlighting,
LSP, plugins, multiple buffers, search & replace, visual mode, clipboard. See
[CHANGELOG.md](CHANGELOG.md) for the version history and [the design notes](#design)
for what's actually inside.

## Install

```sh
cargo install --path .
# or just run from the working tree:
cargo run --release -- /path/to/file
```

Requirements:
- Linux on x86_64
- Rust **1.87** or newer (stable)
- A CPU at the **x86-64-v3** microarchitecture level or above (Haswell, 2013+)

If a built binary won't execute with "Illegal instruction", your CPU is older
than v3. See [`.cargo/config.toml`](.cargo/config.toml) — the v3 baseline is
committed but locally overridable.

### A note on the v3 baseline + shell `RUSTFLAGS`

`.cargo/config.toml` pins `-C target-cpu=x86-64-v3`. **Cargo's precedence rules say
that an exported shell `RUSTFLAGS` overrides config-file rustflags entirely** — they
don't merge. If `echo $RUSTFLAGS` prints anything, that's what's reaching `rustc`,
not what's in this repo.

Cleanest fix is to move whatever you have in shell `RUSTFLAGS` into your *user-level*
Cargo config at `~/.cargo/config.toml`:

```toml
[build]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]   # or whatever you had
```

Then unset the shell variable. After that every project's per-project rustflags
layer correctly. One-off workaround: `env -u RUSTFLAGS cargo build --release`.

## Default keymap (pure vi, no `Ctrl`)

Every binding is defined in [`assets/nib.default.kdl`](assets/nib.default.kdl) and can
be overridden in your user config — see [Config](#config) below.

### Normal mode

| Key                   | Action                          |
| --------------------- | ------------------------------- |
| `h` `j` `k` `l`       | move cursor                     |
| `←` `↓` `↑` `→`       | move cursor (ergonomic alias)   |
| `w` `b`               | word forward / back             |
| `0` `$`               | line start / end                |
| `Home` `End`          | line start / end (alias)        |
| `gg` `G`              | buffer start / end              |
| `i` `a` `I` `A` `o`   | enter insert (variants)         |
| `:`                   | enter command-line mode         |
| `x` · `Delete`        | delete char under cursor        |
| `dd`                  | delete line                     |
| `u`                   | undo                            |
| `U`                   | redo                            |
| `ZZ`                  | save + quit                     |
| `Esc`                 | return to normal (from any mode)|

### Insert mode

Type to insert. `Esc` returns to normal. Arrows + `Home`/`End` move the cursor,
`Tab` inserts a literal tab, `Backspace`/`Delete`/`Enter` work as expected.

### Command-line mode (`:` from normal)

Type a command, press `Enter` to execute. `Esc` cancels. `Backspace` on an empty
cmdline also cancels.

| Cmd         | Action                                          |
| ----------- | ----------------------------------------------- |
| `:w`        | save                                            |
| `:q`        | quit (refuses if there are unsaved changes)     |
| `:q!`       | quit, discarding unsaved changes                |
| `:wq` `:x`  | save and quit                                   |

## Config

User config lives at `$XDG_CONFIG_HOME/nib/nib.kdl` (typically `~/.config/nib/nib.kdl`)
and is **layered on top of** the compiled-in defaults — you only declare what differs.
A broken user config is reported in the status line on first frame rather than locking
you out of the editor.

See [`assets/nib.example.kdl`](assets/nib.example.kdl) for an annotated starting point.

```kdl
behavior {
    tab_width 2
    chord_timeout_ms 700
}

theme {
    background "#1e1e2e"
    status_bg "#cba6f7"
}

keymap "normal" {
    // Leader-style chord.
    "<Space>" {
        w "buffer.save"
        q "editor.quit"
    }
}
```

Chord syntax: bare characters (`h`), or named keys/modifiers inside angle brackets —
`<Esc>`, `<Enter>`, `<F5>`, `<Space>`. Modifiers `C` / `S` / `A` / `M` are accepted
by the parser even though the default config doesn't use any.

The full list of bindable commands lives in [`src/command.rs`](src/command.rs)
(`cursor.*`, `edit.*`, `mode.*`, `buffer.*`, `cmdline.*`, `editor.*`).

## Design

| Module        | Responsibility                                                        |
| ------------- | --------------------------------------------------------------------- |
| `buffer`      | Rope-backed text storage, grapheme cursor, transactional undo/redo    |
| `mode`        | `Mode` enum (Normal / Insert / Command)                               |
| `keymap`      | Chord-trie keymap with `<C-x>` style parser                           |
| `command`     | Named-command registry (`fn(&mut Context)`) including the `:` parser  |
| `config`      | KDL loader, defaults + user overlay                                   |
| `term`        | Linux-only terminal layer (see below)                                 |
| `editor`      | Main loop: poll → dispatch → draw → flush                             |

The terminal layer (`src/term/`) is hand-rolled rather than using `crossterm`:

- **Raw mode** via [`rustix`](https://docs.rs/rustix) `termios` syscalls
  (no `unsafe` in our tree — `unsafe_code = "forbid"` crate-wide).
- **Input parsing** via [`vte`](https://docs.rs/vte) — the ANSI parser that powers
  Alacritty/Wezterm. Wraps `read(2)` and emits typed `Key` events. A small piece of
  bookkeeping disambiguates a bare `Esc` keypress from the start of an escape
  sequence via the `VTIME=1` ~100 ms read window.
- **Output** is direct ANSI escape sequences on top of a cell-diff renderer — a
  back grid is painted each frame, diffed against the front grid, only changed
  cells get written. Same architecture helix/zed use.
- **Resize** via [`signal-hook`](https://docs.rs/signal-hook) registering a `SIGWINCH` flag.
- **XDG paths** via [`etcetera`](https://docs.rs/etcetera).

Total terminal layer is about 500 lines.

## Roadmap

What's plausibly next, in rough order:

- `D` (delete to end of line), `C` (change to end), `^` (first non-blank), `r` (replace one)
- Yank / paste with a clipboard buffer
- Search (`/` and `?`) with regex
- Visual mode (selection + operator)
- Tree-sitter syntax highlighting via [`tree-house`](https://github.com/helix-editor/tree-house)
- Configurable expand-tab (`behavior.expand_tabs`) for spaces-on-`Tab`
- Crash-safe swap files in `$XDG_STATE_HOME/nib/swap/`

## License

GPL-3.0-only. See [LICENSE](LICENSE).

`nib` builds on these crates and would be much smaller without them:
[`ropey`](https://github.com/cessen/ropey),
[`vte`](https://github.com/alacritty/vte),
[`rustix`](https://github.com/bytecodealliance/rustix),
[`kdl-rs`](https://github.com/kdl-org/kdl-rs),
[`clap`](https://github.com/clap-rs/clap),
[`signal-hook`](https://github.com/vorner/signal-hook),
[`etcetera`](https://github.com/xdg-rs/dirs),
[`unicode-segmentation`](https://github.com/unicode-rs/unicode-segmentation),
[`unicode-width`](https://github.com/unicode-rs/unicode-width),
[`anyhow`](https://github.com/dtolnay/anyhow),
[`bitflags`](https://github.com/bitflags/bitflags).
