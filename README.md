<h1 align="center">tesoro</h1>
<p align="center"><code>teso</code> &middot; Obsidian-compatible notes in your terminal.</p>

![tesoro in action](docs/demo.gif)

A vim-style TUI for a folder of markdown. Read styled markdown, follow
`[[wikilinks]]`, edit in place with live preview, search, link your thinking
together, and never leave the terminal. Your notes stay plain `.md` files that
you own.

Built in Rust on [ratatui](https://ratatui.rs). Latest release: **v1.0.0 "Ribbon"**.

## Install

```sh
curl -fsSL https://app.tesoro.ink/install.sh | sh
```

Detects your OS and architecture, fetches the
[latest release](https://github.com/itsgabrieloliver/tesoro/releases/latest),
verifies the sha256, and drops `teso` on your `PATH`.

From source (needs a recent Rust toolchain):

```sh
cargo install --git https://github.com/itsgabrieloliver/tesoro
```

Prebuilt binaries for macOS (Apple Silicon) and Linux (x86_64, aarch64) are on
the [releases page](https://github.com/itsgabrieloliver/tesoro/releases).

## Quick start

```sh
teso ~/notes
```

Point `teso` at any folder of `.md` files. With no argument it falls back to
`$TESORO_VAULT`, then `default_vault` from your config, then the current
directory. A note titled `Welcome` opens first if it exists.

Press `Ctrl-o` to jump between notes, `/` to search, and `Ctrl-p` then `graph`
to see how everything connects.

## Features

- **Live preview editor.** As you edit raw markdown the syntax is concealed and
  rendered in place: heading underlines, **bold**, *italic*, `code`,
  `==highlight==`, `{color:spans}`, wikilinks, tags, callouts, and full tables,
  rendered right on the line you are working on.
- **Wikilinks and backlinks.** `[[Target]]`, `[[Target|Alias]]`,
  `[[Target#Heading]]`, and embeds `![[Target]]`. Follow a link with `Enter`
  (missing notes are created on the spot). A backlinks panel shows what points
  back.
- **Constellation graph.** Every note is a node and every wikilink an edge. The
  most-connected notes float to the top so the shape of your vault is obvious.
- **Find anything fast.** Fuzzy quick-switcher, full-text content search, a tag
  browser, and a command palette, all powered by a smart-case fuzzy matcher.
- **Daily notes and templates.** Open today's note with one key. Templates live
  in `templates/` and render with [minijinja](https://github.com/mitsuhiko/minijinja)
  (`title`, `date`, `time`).
- **Vim editing.** Normal, insert, and visual modes with operators (`dd`, `dw`,
  `diw`, `yy`, `yiw`), visual selection, undo/redo, and system-clipboard yank
  and paste.
- **Buffers and history.** Keep several notes open at once with dirty markers,
  and move back and forward through where you have been.
- **Lives on disk.** A live filesystem watcher reloads notes when they change
  underneath you. Everything is plain markdown with YAML frontmatter
  (`tags`, `aliases`); nothing is locked in a database.

## Keybindings

Leader defaults to `Ctrl`, so the chords below fire on `Ctrl` plus the key. Set
`"leader": "space"` in your config for two-step bindings (press the leader, then
the key).

### Anywhere

| Key | Action |
| --- | --- |
| `Ctrl-o` | Quick-switch between notes |
| `Ctrl-p` | Command palette (`graph`, `open in editor`, ...) |
| `Ctrl-b` | Toggle the backlinks panel |
| `Ctrl-w` | Toggle the sidebar |
| `Tab` | Move focus between sidebar and editor |
| `Ctrl-c` | Save everything and quit |

### Sidebar and reader

| Key | Action |
| --- | --- |
| `j` / `k` | Move selection down / up |
| `Enter` | Open the selected note |
| `/` | Full-text search across the vault |
| `t` | Browse tags |
| `D` | Open (or create) today's daily note |
| `n` | New note |
| `r` / `p` | Rename / pin the selected note |
| `d` `d` | Delete the selected note |

### Editor

| Key | Action |
| --- | --- |
| `i` `a` `A` `o` | Insert, append, append at end of line, open line below |
| `h` `j` `k` `l` | Move by character and line |
| `w` `b` `e` `0` `$` `g` `G` | Word, line, and buffer motions |
| `dd` `dw` `diw` | Delete line, word, inner word |
| `yy` `yw` `yiw` | Yank line, word, inner word |
| `v` | Visual mode |
| `p` / `P` | Paste after / before (system clipboard) |
| `u` / `Ctrl-r` | Undo / redo |
| `Up` / `Down` | Jump to the previous / next wikilink |
| `Enter` | Follow the wikilink under the cursor |
| `Ctrl-l` / `Alt-l` | Wrap word in `[[ ]]` / make an aliased link |
| `Ctrl-e` | Open the full-page preview |
| `Ctrl-s` | Save |
| `:` | Command line |

In insert mode, type `/` at the start of a line for a slash menu (headings,
code, lists, checklists, tables, callouts, highlights, and more).

### Command line

| Command | Action |
| --- | --- |
| `:w` | Save the current note |
| `:wq` / `:x` | Save and close the buffer |
| `:q` / `:q!` | Close the buffer (force) |
| `:qa` / `:qa!` | Save all and quit (force quit) |

## Configuration

Drop a `config.json` in your platform config directory:

- macOS: `~/Library/Application Support/onl.nubo.tesoro/config.json`
- Linux: `~/.config/tesoro/config.json`

```json
{
  "leader": "ctrl",
  "format_on_save": true,
  "save_command": "w",
  "default_vault": "~/notes"
}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `leader` | `ctrl` | Leader key: `ctrl`, `alt`, `space`, `comma`, `backslash`, or any single character |
| `format_on_save` | `true` | Tidy markdown (and align tables) on every save |
| `save_command` | `w` | Extra ex-command word that also saves |
| `default_vault` | none | Vault to open when no path or `$TESORO_VAULT` is given |

A missing or malformed config silently falls back to these defaults.

## Building from source

```sh
git clone https://github.com/itsgabrieloliver/tesoro
cd tesoro
cargo build --release
./target/release/teso ~/notes
```

Run the tests with `cargo test`.

## Stack

`rust` &middot; `ratatui` &middot; `tui-textarea` &middot; `nucleo` &middot; `pulldown-cmark`

Made by [Gabriel Oliver](https://github.com/itsgabrieloliver). Shipped with
[Nubo](https://withnubo.com).

## License

Proprietary. See the package manifest for details.
