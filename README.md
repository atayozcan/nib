# nib

A minimal, highly-configurable modal terminal text editor for Linux.

> The `v3` branch of the old `rusty-editor` repo. The earlier code lives on `main`
> (2021 original) and `v2-rewrite` (2026 stable-Rust rewrite) for reference.

`nib` aims to be small like `vi`, configurable like `kak`, and pick exactly two fights:

- **Vi-style modal editing** that's familiar within five minutes.
- **KDL config** that lets you remap, retheme, and redefine chord trees without
  touching the source.

Out of scope (deliberately, for now): cross-platform support, syntax highlighting, LSP,
plugins, multiple buffers, search/replace.

## Install / run

```sh
cargo run --release -- path/to/file
```

Linux only. The build refuses to compile on other platforms.

If the file does not exist it is created on first save.

## Default keymap

These are bound in `assets/nib.default.kdl` and can be overridden one-by-one in your
user config — you don't need to redeclare bindings you're happy with.

### Normal mode

| Key                | Command                  |
| ------------------ | ------------------------ |
| `h` `j` `k` `l`    | move cursor              |
| `w` `b`            | word forward / back      |
| `0` `$`            | line start / end         |
| `gg` `ge` `G`      | buffer start / end       |
| `i` `a` `I` `A` `o`| enter insert (variants)  |
| `x` `dd`           | delete char / line       |
| `u` `Ctrl-r`       | undo / redo              |
| `Ctrl-s`           | save                     |
| `Ctrl-q` `Esc`     | quit (Esc returns to normal in other modes) |

### Insert mode

Type to insert. `Esc` returns to normal. `Backspace`, `Delete`, `Enter`, `Ctrl-s` are
also bound.

## Config

User config lives at `$XDG_CONFIG_HOME/nib/nib.kdl` (typically `~/.config/nib/nib.kdl`).
It is layered *on top of* the compiled-in defaults — only declare what differs.

```kdl
behavior {
    tab_width 2
    line_numbers #true
    chord_timeout_ms 700
}

theme {
    background "#1e1e2e"
    status_bg "#cba6f7"
}

keymap "normal" {
    // Remap j/k like an English typist who hates the QWERTY home row.
    n "cursor.down"
    e "cursor.up"

    // Add a leader-style chord.
    "<Space>" {
        w "buffer.save"
        q "editor.quit"
    }
}
```

Chord syntax: bare characters (`h`), or named keys/modifiers inside angle brackets —
`<Esc>`, `<Enter>`, `<C-s>`, `<C-S-Tab>`, `<F5>`, `<Space>`.

The full list of commands lives in `src/command.rs` (`cursor.*`, `edit.*`, `mode.*`,
`buffer.*`, `editor.*`).

## Design

| Module        | Responsibility                                                        |
| ------------- | --------------------------------------------------------------------- |
| `buffer`      | Rope-backed text storage with grapheme cursor + transactional undo    |
| `mode`        | `Mode` enum (Normal / Insert / Command)                               |
| `keymap`      | Chord-trie keymap with `<C-x>` style parsing                          |
| `command`     | Named-command registry (`fn(&mut Context)`)                           |
| `config`      | KDL loader, defaults + user overlay                                   |
| `term`        | Linux-only terminal layer (see below)                                 |
| `editor`      | Main loop: poll → dispatch → draw → flush                             |

The terminal layer (`src/term/`) is hand-rolled rather than using `crossterm`:

- **Raw mode** via [`rustix`](https://docs.rs/rustix) `termios` syscalls
  (no `unsafe` in our tree; `unsafe_code` is `forbid`ed crate-wide).
- **Input parsing** via [`vte`](https://docs.rs/vte) — the same ANSI parser that
  powers alacritty/wezterm. Wraps `read(2)` and emits typed `Key` events.
- **Output** is direct ANSI escape sequences over a cell-diff renderer — a back grid
  is painted each frame, diffed against the front grid, and only changed cells get
  written. Same architecture helix/zed use.
- **Resize** via `signal-hook` registering a `SIGWINCH` flag.
- **XDG paths** via `etcetera`.

Total terminal layer: about 350 lines.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
