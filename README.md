# musictag

A music metadata editor written in Rust with a TUI, designed for audio files of any format — FLAC, OPUS, M4A and more! Uses Vim keys.

## Features

- Browse directories and edit metadata one by one
- Batch editing: select multiple files and edit them all at once
- Delete files directly from the app
- Set a default folder that opens on startup
- Auto-detects system language (Spanish/English)
- Supports MP3, FLAC, OGG, Opus, M4A/AAC, WAV, APE, WMA
- Nerd Font icons

## Keybindings

### Browse Mode

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` | Go to parent directory |
| `l` / `→` / `Enter` | Open folder or edit file |
| `Esc` | Go to parent directory |
| `g` / `Home` | Go to first item |
| `G` / `End` | Go to last item |
| `a` / `Space` | Toggle selection |
| `v` | Select all audio files |
| `A` | Edit all selected files (batch) |
| `d` | Clear selection |
| `f` / `/` | Filter files |
| `r` | Reload directory |
| `C` | Set current folder as default |
| `x` / `Delete` | Delete file(s) |
| `q` | Quit |

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

## Install

```bash
# From source (requires Rust)
git clone https://github.com/youruser/musictag.git
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
