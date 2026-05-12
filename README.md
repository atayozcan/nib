# rusty-editor

A small terminal text editor written in Rust. The `v2-rewrite` branch is a from-scratch
rewrite of the original 2021 code using current crates and idioms.

## Run

```
cargo run --release -- path/to/file.txt
```

If the file does not exist it will be created on the first save.

## Keys

| Key            | Action                              |
| -------------- | ----------------------------------- |
| Ctrl+S         | Save                                |
| Ctrl+Q · Esc   | Quit                                |
| Arrows         | Move cursor (grapheme-aware)        |
| Home · End     | Start / end of line                 |
| Backspace      | Delete left, crosses line boundary  |
| Delete         | Delete right (whole grapheme)       |
| Enter          | Insert newline                      |

## Design

- **`buffer`** — Text storage as a [`ropey::Rope`](https://docs.rs/ropey) so insert/delete
  in the middle of a large file is `O(log n)` rather than `O(n)` for a flat `String`.
  Cursor position is `(line, grapheme-column)`, not byte offset — so combining accents
  and ZWJ emoji advance the cursor by what a human sees as one character.
- **`terminal`** — An RAII guard that enables raw mode + alternate screen on construction
  and restores the terminal on `Drop`. Survives panics, so a bug doesn't strand you in
  a broken terminal.
- **`editor`** — The event loop and key dispatch. Pure state mutation; no rendering.
- **`ui`** — Pure rendering. Reads `Editor` state and produces a frame.
- **`cli`** — `clap` derive for argument parsing.

`unsafe_code` is forbidden crate-wide.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
