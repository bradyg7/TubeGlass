# TubeGlass

A YouTube desktop client and media player in Rust (egui). No account, no API key.

## Features
- **Channels:** follow channels by `@handle`, link or id; per-channel Videos / Live / Playlists / About
  tabs with infinite scroll and in-channel search. Import / export subscriptions (NewPipe JSON,
  YouTube Takeout CSV, OPML).
- **Feed:** cached on disk and shown instantly; *Refresh* updates it. "NEW" markers and a New filter.
- **Search:** videos, channels and playlists, suggestions, recent searches, sort / date / length filters.
- **Player:** quality, speed 0.5–2×, chapters with hover previews, captions, SponsorBlock skipping,
  audio-only mode with a visualiser, comments with replies, related videos, picture-in-picture,
  resume where you left off, history.
- **Queue:** *Play all*, shuffle, repeat, autoplay, up-next panel, sleep timer.
- **Offline:** resumable downloads (pause / resume / quality / audio-only), multi-select delete,
  offline playback.
- **Local files:** open any audio or video file (Ctrl+O, drag-and-drop, or *Open With*).
- **Keep offline:** mark any playlist (or Liked songs) *Keep offline* and new items download by
  themselves. Albums and playlists group into one folder row in Downloads; captions and lyrics are
  saved with each download. The app notices when you are offline and opens your downloads.
- **Audio:** equalizer, volume levelling, skip silence (spoken audio), gapless playback with optional
  crossfade, mini player (`I`), drag-to-reorder queue that can be saved as a playlist.
- **Captions:** auto-translate to 13 languages; a translation is also kept with downloads.
- **Podcasts:** subscribe to any RSS feed (search the directory, paste a feed address or an Apple
  Podcasts link). Episodes play in the normal player with resume, speed and sleep timer, download
  for offline listening, and new ones are marked and refreshed in the background.
- **Views:** Downloads, Clips and Podcasts can be shown as a grid, a compact grid or a list, the
  same switch as on the video pages (the choice is shared and remembered).
- **Insights:** watch / listen time per day, top channels and artists (local only). Home can be
  ordered *For you*, with *Not interested* / *Recommend less from this channel* on any video.
- **Looks:** dark / light / system theme, text size, card density, custom accent colour, and
  grid / compact / list views.
- **Tools:** searchable transcript, notes and bookmarks (dots on the seek bar), and clipping a
  section to MP4, M4A or GIF. A storage manager frees space from finished or old downloads.
- **Safety net:** back up / restore everything, per-channel notifications and playback speed,
  *Check YouTube access* diagnostics, and a daily CI canary that opens an issue when YouTube breaks.
- **Desktop:** media keys / MPRIS, optional "channel went live" notifications.

Requires `ffmpeg` and `ffprobe` on PATH. Run with `cargo run --release`.

## Install

Download the file for your system from the [latest release](https://github.com/flawlesscode254/TubeGlass/releases/latest):

| System | File | How |
| --- | --- | --- |
| Linux (any) | `TubeGlass-<version>-linux-<arch>.AppImage` | `chmod +x` it and run; it updates itself from inside the app |
| Debian / Ubuntu | `tubeglass_<version>_<amd64\|arm64>.deb` | `sudo apt install ./tubeglass_*.deb` |
| Linux (other) | `TubeGlass-<version>-linux-<arch>.tar.gz` | unpack and run `./install.sh` (no root needed) |
| Arch Linux | `packaging/PKGBUILD` | `makepkg -si` |
| Windows 10/11 | `TubeGlass-<version>-windows-x86_64.zip` | unzip and run `TubeGlass.exe` |
| macOS 11+ | `TubeGlass-<version>-macos-<aarch64\|x86_64>.dmg` | drag TubeGlass to Applications; the first time, right-click it and choose *Open* (the app is not notarised) |

TubeGlass needs **ffmpeg** on your system. If it is missing the app says so and shows the command:
`sudo apt install ffmpeg`, `brew install ffmpeg` or `winget install ffmpeg`.

Linux gets everything (media keys through MPRIS, desktop notifications, "Open with"). macOS and
Windows do not have media-key integration yet, and macOS double-click "Open with" is not wired up
(use *Open file…* in the app).

### Updates

TubeGlass checks this repository's releases a few seconds after it starts (and every few hours
while it is open). When a newer version exists a bar appears with **Update**, **What's new** and
**Skip this version**. *Update* downloads the right file for your system and checks it; an
AppImage replaces itself and offers a restart, other formats open in your installer or file
manager. Turn the check off in *Settings*.

### Build from source

```
cargo run --release          # needs ffmpeg and a Rust toolchain
./install.sh                 # build and install for the current user (Linux)
```

On Linux the build needs `pkg-config`, `libasound2-dev`, `libxkbcommon-dev` and `libwayland-dev`.

## Releasing

1. Raise `version` in `Cargo.toml` and commit it.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. The *Release* workflow builds every platform and publishes the release with checksums.
   Pushing the branch `release-dryrun` (or Actions > Release > *Run workflow*) builds everything
   without publishing; the files are attached to the run.

## Keys
`Space/K` play·pause, `←/→` ±5s, `J/L` ±10s, `↑/↓` volume, `M` mute, `[ ]` speed, `N/P` next·previous,
`Q` queue, `A` audio-only, `C` comments, `F` fullscreen, `Esc` back (keeps playing in PiP),
`Ctrl+O` open file, `Ctrl+B` sidebar, `?` shortcut sheet.

## Code layout

```
src/
  main.rs        entry point and shared imports
  app/           application state and logic (playing, queue, downloads, navigation, messages)
  ui/            everything that draws: pages, widgets, theme, icons
  data/          settings, files on disk, types shared across the app
  youtube/       YouTube and YouTube Music extraction (no account, no API key)
  playback/      ffmpeg player, local range proxy, local files, MPRIS media keys
  library/       downloads, clips, podcasts, subscriptions import, statistics
  platform.rs    the few things that differ between Linux, macOS and Windows
  util.rs        small helpers
build.rs         embeds the icon in the Windows executable
packaging/       .deb, AppImage, tarball, macOS .app/.dmg, Windows zip, PKGBUILD, icons
.github/         CI, release and daily YouTube-canary workflows
```
