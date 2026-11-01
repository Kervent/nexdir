# Your first contribution

A step-by-step path from zero to a merged pull request. The rules are in
[CONTRIBUTING.md](../../CONTRIBUTING.md); this page is the practical walkthrough.

## 1. Find something to work on

- Issues labelled [`good first issue`](../../../../issues?q=is%3Aopen+label%3A%22good+first+issue%22)
  are small and well described; [`help wanted`](../../../../issues?q=is%3Aopen+label%3A%22help+wanted%22)
  are welcome contributions of any size.
- Docs fixes need no issue — just open a pull request.
- For anything larger, open an issue first and agree on the approach.

Comment on the issue that you'd like to work on it.

## 2. Set up

Fork the repository on GitHub, then:

```bash
git clone https://github.com/<you>/nexdir.git
cd nexdir
npm ci
npm run app:dev
```

Prerequisites (Node.js, Rust, Visual Studio Build Tools) are listed in [setup.md](setup.md).
The first Rust build takes several minutes. The UI needs the Rust backend to show real
folders; `npm run dev` alone serves the interface in a browser, but without a backend it cannot
list files. For UI logic, the Vitest tests with a fake backend (`npm test`) are the quickest loop.

## 3. Find your way around

| You want to change… | Look in |
|---|---|
| What a button, list or dialog looks like | `src/features/<area>/`, `src/components/`, `src/styles/` |
| Keyboard shortcuts | `src/app/App.tsx`, `src/features/settings/ShortcutsDialog.tsx` |
| How a file operation behaves | `src-tauri/src/operations/` (logic), `src-tauri/src/platform/` (Win32 calls) |
| Path and input validation | `src-tauri/src/security/` |
| Search, storage analysis, duplicates | `src-tauri/src/search/`, `src-tauri/src/analytics/`, `src/features/tools/` |
| Calls between UI and backend | `src/services/api.ts` ↔ `src-tauri/src/commands/` |
| Discord tools | `tools/discord-setup/`, `tools/discord-notify/` |

The big picture is in [architecture/overview.md](../architecture/overview.md); the security
model in [security/threat-model.md](../security/threat-model.md).

## 4. Make the change

```bash
git switch -c fix/short-description
```

- Keep it focused: one problem per pull request.
- Add or update tests next to the code you change:
  - TypeScript: `*.test.ts(x)` files with Vitest; the UI test uses a fake backend.
  - Rust: `#[cfg(test)]` modules; anything that touches files works in a `tempfile::tempdir()`.
- **Never** point tests or manual experiments at your own documents, pictures or other personal
  folders. Create test folders and files instead.
- New user-visible behaviour: add a line under `## [Unreleased]` in `CHANGELOG.md`.

## 5. Check before you push

```bash
npm run typecheck
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

If Windows blocks the Rust test executable (*os error 5* or *32*, e.g. from antivirus scanning
it), build tests into a separate folder:
`cargo test --manifest-path src-tauri/Cargo.toml --release --target-dir target-perf`.

## 6. Open the pull request

Commit with a [Conventional Commit](https://www.conventionalcommits.org/) message
(`fix(search): match file names case-insensitively`), push to your fork and open a pull request.
Fill in the template — especially *How was this tested?* and the safety checklist.

CI runs automatically (for first-time contributors, a maintainer starts it). A maintainer
reviews; changes in `operations/`, `security/` or `platform/` get an extra careful review. Expect
questions — they're part of the process, not a judgement.

## Getting help

Ask in Discord **#contribution-help** or comment on the issue. See also the
[community guidelines](../community/guidelines.md).
