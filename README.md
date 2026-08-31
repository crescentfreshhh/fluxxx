# fluxxx

A lean, snappy **Windows** desktop player for **Xtream Codes** IPTV — built to be an
excellent **Discord "Go Live"** capture target.

- **Live TV** with a full **EPG timeline grid**
- **Multi-provider** support — each provider toggleable on/off (credentials retained)
- **Curation-first**: a setup wizard lets you enable only the countries/categories you
  want, *before* any heavy EPG fetch. Disabled providers/groups are **fully excluded** —
  no catalog refresh, no EPG download, no cache. Taming a huge multi-country channel dump
  is a first-class goal.
- **Discord mode**: borderless, preset-sized window with clean chrome and predictable
  audio, so screen-sharing it via Go Live looks and sounds right.

> Discord has no supported API to push a video feed into a channel, so fluxxx is designed
> to be screen-shared (Go Live), not to "connect" to Discord directly.

## Tech

| Layer     | Choice                                             |
| --------- | -------------------------------------------------- |
| Shell     | [Tauri v2](https://tauri.app) (small, low-memory)  |
| Core      | Rust (`fluxxx-core` crate — pure, unit-tested)     |
| Playback  | libmpv (hardware-decoded MPEG-TS/HLS) — _upcoming_ |
| UI        | Vanilla TypeScript + Vite                          |
| Storage   | SQLite (catalog + EPG cache) — _upcoming_          |

## Repository layout

```
crates/core/      fluxxx-core: Xtream parsing, country inference, curation (Linux-testable)
src-tauri/        Tauri app crate (Windows GUI, built in CI)
src/              Frontend (TypeScript + Vite)
scripts/          Icon generation
.github/workflows build.yml — Linux core tests + Windows exe/installer build
```

## Building

The Windows `.exe` is produced by CI (this project is developed in a Linux environment
that can't compile a Windows GUI). Every push to `main` builds:

- **Portable** `fluxxx-portable-windows-x64.zip` (no install)
- **Installer** NSIS `*-setup.exe`

Both are attached to GitHub Releases on `v*` tags, and available as workflow artifacts on
every run.

### Local development (on Windows)

Prerequisites: [Rust](https://rustup.rs), Node 20+, and the
[Tauri v2 prerequisites](https://tauri.app/start/prerequisites/) (WebView2 is preinstalled
on Windows 10/11).

```bash
npm install
npm run tauri dev      # run the app with hot-reload
npm run tauri build    # produce exe + installer
```

### Core logic tests (any platform)

```bash
cargo test -p fluxxx-core
```

## Status

Phase 0 — project scaffold, CI pipeline, and the pure-logic core (Xtream response
parsing, country inference, curation rules) with unit tests. See the roadmap in the plan
for subsequent phases (provider management, curation wizard, channel UI, EPG grid,
libmpv playback, Discord mode, packaging).

## License

MIT
