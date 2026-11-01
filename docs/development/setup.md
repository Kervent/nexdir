# Development setup

## Prerequisites (Windows 10/11 x64)

| Tool | Version used | Install |
|---|---|---|
| Node.js | 22 LTS or newer | <https://nodejs.org> |
| Rust | stable, `x86_64-pc-windows-msvc` | `winget install Rustlang.Rustup` |
| Visual Studio Build Tools | 2022, workload *Desktop development with C++* | `winget install Microsoft.VisualStudio.2022.BuildTools` then add the C++ workload |
| WebView2 Runtime | included in Windows 11 | <https://developer.microsoft.com/microsoft-edge/webview2/> |
| Git | any recent | `winget install Git.Git` |

## Commands

```bash
npm ci                     # install exact dependency versions
npm run app:dev            # start Vite + the Tauri app with hot reload
npm run typecheck          # TypeScript
npm test                   # Vitest (frontend)
cargo test --manifest-path src-tauri/Cargo.toml             # Rust tests
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run app:build          # release exe + NSIS and MSI installers
npm run branding           # regenerate all logo/icon files from the SVG master
powershell -ExecutionPolicy Bypass -File tools/release/package.ps1 -Clean   # release files + SHA256SUMS in release-out/
```

Build outputs:

- `src-tauri/target/release/nexdir.exe`
- `src-tauri/target/release/bundle/nsis/Nexdir_<version>_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/Nexdir_<version>_x64_en-US.msi`

The first `tauri build` downloads NSIS and WiX into the Tauri cache.

## Tests

- Rust tests create everything in temporary directories (`tempfile`). Some tests create
  NTFS junctions with `mklink /J` (no admin rights needed).
- The test that uses the real Recycle Bin is ignored by default:
  `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored`.
- Frontend tests replace the Tauri backend with an in-memory fake (`src/app/App.test.tsx`).

## Project layout

```text
src/                React UI (app/, components/, features/, lib/, services/, styles/, types/)
src-tauri/          Rust backend, Tauri config, capabilities, icons
assets/branding/    Logo master (SVG) and generated assets
tools/branding/     Asset generator
tools/discord-setup Discord server setup tool (one-time)
tools/discord-notify GitHub → Discord announcements (runs in GitHub Actions)
.github/            CI, release, announcement and label workflows; issue/PR templates; labels.json
docs/               Architecture, development, security, release docs
```
