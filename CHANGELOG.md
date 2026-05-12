# Changelog

All notable changes to `nib`. Format follows [Keep a Changelog](https://keepachangelog.com/)
loosely; this is a single-author hobby editor and the version numbers below describe
*design generations* more than they describe semver.

## [0.4.1] — 2026-05-13

### Fixed

- `TerminalGuard::enter` called `tcsetattr` with `TCSAFLUSH`, which silently
  discards any input the user typed between `exec(2)` and the raw-mode switch
  ("type-ahead"). The visible effect: open a large file, mash keys while it's
  loading, and those keys vanish. Now uses `TCSANOW`, which keeps the buffered
  input. (`TerminalGuard::Drop` still uses `TCSAFLUSH` — correct on exit,
  since we want any half-typed garbage to *not* leak into the shell prompt.)

  Surfaced while writing a load-time benchmark that scripted `nib FILE` with
  an immediate `:q` over a PTY: it worked for small files but timed out for
  larger ones, because nib's startup took longer than the harness's
  write-bytes delay, putting the `:q\r` in the cooked-mode input queue
  *before* the `TCSAFLUSH` ran, after which it was gone.

## [0.4.0] — 2026-05-12

### Added

- **aarch64-unknown-linux-gnu** as a supported target, with a `cortex-a520`
  codegen baseline tuned for the Radxa Orion O6 / Cix CD8180 (Armv9.2-A,
  A720+A520 big.LITTLE). Same source, no `unsafe`, all 12 tests pass on
  emulated Armv9-A via `qemu-aarch64-static -cpu max`.
- `.cargo/config.toml` gains a qemu `runner` for the aarch64 target so
  `cargo run / cargo test --target aarch64-...` work transparently on an
  x86_64 host.
- README documents the cross-compile workflow from an x86_64 dev box to the
  O6.

### Changed

- `cfg` gate in `src/main.rs` now accepts `x86_64` or `aarch64`; other
  architectures still refuse to compile.
- `Cargo.toml` `repository` URL points at the renamed GitHub repo
  (`atayozcan/nib`) instead of the redirected `atayozcan/rusty-editor`.

## [0.3.0] — 2026-05-12

First release worth calling that. A from-scratch rewrite of the v0.2 codebase
under a new design philosophy.

### Added

- Pure **vi-style modal editing** — `hjkl`, `i a o I A`, `:w :q :wq :q! :x`, `ZZ`, `dd`, `gg G`, `u U`. No `Ctrl-*` bindings.
- Arrow keys, `Home`, `End`, `Delete` bound alongside `hjkl` in Normal mode and as the
  only navigation keys in Insert mode.
- **Command-line mode** (`:` from Normal): parses `w / q / q! / wq / x`; `q` refuses
  to quit if the buffer is dirty.
- **KDL config** loaded from `$XDG_CONFIG_HOME/nib/nib.kdl`, layered over compiled-in
  defaults. Themable foreground / background / status / cmdline. Behaviour knobs for
  `tab_width`, `line_numbers`, `chord_timeout_ms`. Chord-trie keymaps with `<C-x>` style
  modifier syntax.
- **Transactional undo/redo** with grapheme-cluster cursor.
- **Hand-written terminal layer**: raw mode via `rustix` (no `unsafe` in tree), input
  parsed via `vte`, output via direct ANSI on top of a cell-diff renderer. No
  `crossterm`, no `ratatui`.
- **x86-64-v3 codegen baseline** pinned in `.cargo/config.toml`.
- 12 unit tests covering buffer ops, undo/redo, motion (incl. repeated-word regression),
  keymap parsing, and chord-trie lookup.

### Fixed

- Bare `Esc` keypress no longer disappears into `vte`'s escape-state buffer.
- `save()` no longer rewrites the extension when computing its tmp file
  (`foo.tar.gz` → `foo.tar.gz.nib~`, not `foo.tar.nib~`); also `fsync`s before the
  rename so a power cut between write and swap doesn't lose data.
- Word motions (`w` / `b`) on lines with repeated words now use offset tracking
  instead of `str::find`, fixing the "second `foo` jumps back to the first" bug.
- Off-by-one in `Buffer::line_col_to_char_idx` that could panic inside `ropey` when
  the cursor crossed the buffer's last line.

### Removed

- All `Ctrl-*` bindings from the default keymap (`<C-s>`, `<C-q>`, `<C-r>`).
- `ge` from the default `g`-prefix — it was bound to "goto buffer end" which
  conflicted with vi's canonical "previous word end" meaning and was redundant
  with `G` anyway.
- Three unused dependencies (`crossbeam-channel`, `thiserror`, `tempfile`) and one
  no-op `[target.'cfg(not(linux))'.dependencies]` block in `Cargo.toml`.

### Known issues / out of scope

- No yank/paste — `dd` deletes text into the void.
- No syntax highlighting, LSP, multiple buffers, splits, or search.
- No visual / selection mode.
- The chord timeout is configurable but a single key never times out — so binding both
  a single key and a chord starting with it gives the single-key binding precedence.

## [0.2.0] — 2026-05-12

Lives on the `v2-rewrite` branch. A from-scratch rewrite of the 2021 code using
current crates and stable Rust 2024:

- `structopt` → `clap` v4 derive
- `tui 0.15` → `ratatui 0.30`
- `termion` → `crossterm 0.29`
- `String` buffer → `ropey` rope with `(line, grapheme-column)` cursor
- `breakpoint()` exit intrinsic → RAII terminal guard, panic-safe restore
- Wildcard versions → pinned majors
- Removed 128 lines of dead copy-pasted `util.rs` code

5 unit tests on the buffer. CI matrix on Linux/macOS/Windows.

## [0.1.0] — 2021-XX

The original 2021 hobby project. Nightly-only (`#![feature(core_intrinsics)]`),
terminal text editor that quit via `unsafe { breakpoint() }`. Did not work on stable
Rust. See git history on the `main` branch's earliest commits for what was there.

[0.4.1]: https://github.com/atayozcan/nib/releases/tag/v0.4.1
[0.4.0]: https://github.com/atayozcan/nib/releases/tag/v0.4.0
[0.3.0]: https://github.com/atayozcan/nib/releases/tag/v0.3.0
