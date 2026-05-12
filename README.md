# nib

A minimal, highly-configurable modal terminal text editor for Linux x86_64.

> The `v3` branch of the old `rusty-editor` repo. The earlier code lives on `main`
> (2021 original) and `v2-rewrite` (2026 stable-Rust rewrite) for reference.

`nib` is small like `vi`, configurable like `kak`, and picks exactly two fights:

- **Pure vi-style modal editing** — `hjkl`, `i/a/o`, `:w`, `ZZ`. No `Ctrl-*` bindings.
- **KDL config** that lets you remap, retheme, and redefine chord trees without
  touching the source.

Out of scope (deliberately, for now): cross-platform support, syntax highlighting,
LSP, plugins, multiple buffers, search/replace, visual mode.

## Install / run

```sh
cargo run --release -- path/to/file
```

Builds and runs on **Linux x86_64 only** — the source `cfg`-refuses other platforms.
Codegen baseline is **x86-64-v3** (Haswell-and-newer; needs AVX2/BMI2/FMA).

If the file does not exist it is created on first save.

### A note on the v3 baseline and your shell `RUSTFLAGS`

The repo pins `-C target-cpu=x86-64-v3` in [`.cargo/config.toml`](.cargo/config.toml).
**However**, Cargo's precedence rules say that an exported shell `RUSTFLAGS` overrides
config-file rustflags entirely (they don't merge). If `echo $RUSTFLAGS` prints anything,
that anything is silently winning over our config.

Cleanest fix: move whatever you have in shell `RUSTFLAGS` into your user-level Cargo
config at `~/.cargo/config.toml`:

```toml
[build]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]   # or whatever you had
```

Then unset the shell var. Per-project rustflags layer correctly after that.

Workarounds for a one-off: `env -u RUSTFLAGS cargo build --release`, or include
`-C target-cpu=x86-64-v3` in your existing `RUSTFLAGS` yourself.

## Default keymap (pure vi, no `Ctrl`)

Every binding is defined in [`assets/nib.default.kdl`](assets/nib.default.kdl) and can
be overridden in your user config — see [Config](#config) below.

### Normal mode

| Key                    | Action                          |
| ---------------------- | ------------------------------- |
| `h` `j` `k` `l`        | move cursor                     |
| `w` `b`                | word forward / back             |
| `0` `$`                | line start / end                |
| `gg` `ge` `G`          | buffer start / end              |
| `i` `a` `I` `A` `o`    | enter insert (variants)         |
| `:`                    | enter command-line mode         |
| `x`                    | delete char under cursor        |
| `dd`                   | delete line                     |
| `u`                    | undo                            |
| `U`                    | redo                            |
| `ZZ`                   | save + quit                     |
| `Esc`                  | return to normal (from any mode)|

### Insert mode

Type to insert. `Esc` returns to normal. `Backspace`, `Delete`, `Enter` work as
expected.

### Command-line mode (`:` from normal)

Type a command, press `Enter` to execute. `Esc` cancels. `Backspace` on an empty
cmdline also cancels.

| Cmd         | Action                                |
| ----------- | ------------------------------------- |
| `:w`        | save                                  |
| `:q`        | quit (refuses if there are unsaved changes) |
| `:q!`       | quit, discarding unsaved changes      |
| `:wq` `:x`  | save and quit                         |

## Config

User config lives at `$XDG_CONFIG_HOME/nib/nib.kdl` (typically `~/.config/nib/nib.kdl`).
It's layered *on top of* the compiled-in defaults — only declare what differs.

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
    // Remap j/k for an English typist who hates the QWERTY home row.
    n "cursor.down"
    e "cursor.up"

    // Leader-style chord.
    "<Space>" {
        w "buffer.save"
        q "editor.quit"
    }
}
```

Chord syntax: bare characters (`h`), or named keys/modifiers inside angle brackets —
`<Esc>`, `<Enter>`, `<F5>`, `<Space>`. Modifiers `C`/`S`/`A`/`M` are accepted by the
parser if you want to bind them, even though the default config doesn't use any.

The full list of commands lives in `src/command.rs` (`cursor.*`, `edit.*`, `mode.*`,
`buffer.*`, `cmdline.*`, `editor.*`).

## Design

| Module        | Responsibility                                                        |
| ------------- | --------------------------------------------------------------------- |
| `buffer`      | Rope-backed text storage with grapheme cursor + transactional undo    |
| `mode`        | `Mode` enum (Normal / Insert / Command)                               |
| `keymap`      | Chord-trie keymap with `<C-x>` style parsing                          |
| `command`     | Named-command registry (`fn(&mut Context)`) including the `:` parser  |
| `config`      | KDL loader, defaults + user overlay                                   |
| `term`        | Linux-only terminal layer (see below)                                 |
| `editor`      | Main loop: poll → dispatch → draw → flush                             |

The terminal layer (`src/term/`) is hand-rolled rather than using `crossterm`:

- **Raw mode** via [`rustix`](https://docs.rs/rustix) `termios` syscalls
  (no `unsafe` in our tree; `unsafe_code` is `forbid`ed crate-wide).
- **Input parsing** via [`vte`](https://docs.rs/vte) — the ANSI parser that powers
  alacritty/wezterm. Wraps `read(2)` and emits typed `Key` events.
- **Output** is direct ANSI escape sequences on top of a cell-diff renderer — a
  back grid is painted each frame, diffed against the front grid, only changed
  cells are written. Same architecture helix/zed use.
- **Resize** via `signal-hook` registering a `SIGWINCH` flag.
- **XDG paths** via `etcetera`.

Total terminal layer: about 350 lines.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
