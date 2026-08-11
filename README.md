# musictag

A music metadata editor written in Rust with a TUI, designed for audio files of any format — FLAC, OPUS, M4A and more! Vim keys by default, with full keybinding, color and border customization.

## Features

- Browse directories and edit metadata one by one or in batch
- Delete files directly from the app
- Cover art preview and cover art editor (set a custom image)
- Extract embedded cover art to `~/.config/musictag/covers/`
- MusicBrainz search: find metadata (title, artist, album, year, genre, comments...) and apply it
- Open MusicBrainz search in your browser with one key
- Set a default folder that opens on startup
- Auto-detects system language (Spanish/English)
- Mouse support (click, double-click, scroll, right-click selection)
- Fully customizable config: colors, keybindings, border styles — reloaded on the fly
- Supports MP3, FLAC, OGG, Opus, M4A/AAC, WAV, APE, WMA
- Nerd Font icons

## Configuration

The config file lives at `~/.config/musictag/config`. It uses a simple `key=value` format. Whenever you save it (with `C` in the app) a commented example theme is appended automatically.

```ini
default_dir=/home/you/Music
show_preview=true
nav=vim            # vim or arrows

# Colors — commented by default, uncomment to change
# color.background=#1e1e2e
# color.folder=#89dceb
# color.selected=#cba6f7
# color.error=#f38ba8
# color.metadata_label=#cba6f7
# ...

# Borders — single, rounded, double, thick, none
border=rounded
# border.preview=double
# border.filter=rounded
# border.delete=double
# border.cover=rounded
# border.input=single

# Keybindings — see the key table below for action names
# key.quit=q,Q
# key.down=j
# key.up=k
# key.enter=enter,space
# key.extract_cover=e
```

Every color, border and keybinding can be overridden from here and it **reloads automatically** when you edit the file — no restart needed. Changing a keybinding updates the help bar and hints too.

### Action names

| Action | Default keys |
|--------|--------------|
| `quit` | `q`, `Q` |
| `toggle_nav` | `N` |
| `toggle_help` | `/` (vim) / `h` (arrows) |
| `down` / `up` | `j` / `k` (vim), arrows |
| `left` / `right` | `h` / `l` (vim), arrows |
| `home` / `end` | `g` / `G` (vim), `Home` / `End` (arrows) |
| `enter` | `Enter` |
| `escape` | `Esc` |
| `toggle_select` | `a`, `Space` |
| `filter` | `f` |
| `reload` | `r` |
| `apply_all` | `A` |
| `apply_selected` | `V`, `v` |
| `clear_selection` | `d` |
| `delete` | `x`, `Delete` |
| `save_config` | `C` |
| `set_cover_art` | `c` |
| `reset_colors` | `R` |
| `toggle_preview` | `P` |
| `extract_cover` | `e` |
| `next_field` / `prev_field` | `Tab` / `Shift+Tab` |
| `save_next` | `Ctrl+S` |
| `clear_field` | `Ctrl+U` |
| `apply_to_all` | `Ctrl+C` |
| `apply_mb` | `Ctrl+G` |
| `mb_prev` / `mb_next` | `PageUp` / `PageDown` |
| `mb_browser` | `Ctrl+O` |
| `confirm_yes` / `confirm_no` | `y`, `Y` / `n`, `N` |
| `backspace` / `delete_char` | `Backspace` / `Delete` |

Valid key values: single characters (`q`, `A`, `/`, ` `), `enter`, `esc`, `tab`, `shift-tab`, `backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `space`, and `ctrl+x`. Separate multiple keys with commas.

## Keybindings (defaults)

### Browse Mode (vim scheme)

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `h` | Go to parent directory |
| `l` / `Enter` | Open folder or edit file |
| `Esc` | Go to parent directory |
| `g` / `G` | First / last item |
| `a` / `Space` | Toggle selection |
| `v` | Select all audio files |
| `A` | Edit all selected files (batch) |
| `d` | Clear selection |
| `f` | Filter files |
| `r` | Reload directory |
| `c` | Set cover art |
| `e` | Extract cover art to config folder |
| `x` / `Delete` | Delete file(s) |
| `P` | Toggle preview panel |
| `N` | Switch between vim / arrow keys |
| `C` | Save config |
| `R` | Reset colors to configured theme |
| `/` | Toggle help |
| `q` | Quit |

Press `N` for the arrows scheme: `↑`/`↓` move, `←`/`→` parent/open, `Home`/`End` first/last, `f` or `/` filter, `h` help.

### Edit Mode

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous field |
| `↑` / `↓` | Next / previous field |
| `←` / `→` | Move cursor in text |
| `Home` / `End` | Start / end of text |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `Ctrl+U` | Clear current field |
| `Ctrl+G` | Apply MusicBrainz suggestion |
| `PageUp` / `PageDown` | Navigate MusicBrainz results |
| `Ctrl+O` | Open MusicBrainz search in browser |
| `Enter` | Save and return to browse |
| `Ctrl+S` | Save and edit next file |
| `Ctrl+C` | Apply current field to all selected |
| `Esc` | Cancel without saving |

### Filter Mode

| Key | Action |
|-----|--------|
| Type to filter | Live filter as you type |
| `Enter` | Apply filter |
| `Backspace` | Delete character |
| `Ctrl+U` | Clear filter |
| `Esc` | Cancel filter |

## Mouse

- Left click: select item
- Double left click: open folder / edit file
- Right click: select / toggle selection
- Scroll wheel: navigate
- Clicking a form field in edit mode moves the cursor there
- Clicking outside a popup cancels it

## Install

```bash
# From source (requires Rust)
git clone https://github.com/NateInkDemon3070/musictag.git
cd musictag
chmod +x install.sh
./install.sh
```

Or with makepkg on Arch/Artix:

```bash
makepkg -si
```

## License

MIT
