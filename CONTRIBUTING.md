# Contributing to Nexdir

Thanks for your interest in Nexdir! Everyone is welcome to fork the repository and open
pull requests. Write access to the main repository is granted only by the project owner.

New here? Follow [your first contribution](docs/development/first-contribution.md) step by step.

## Ways to contribute

Code is only one way to help. Beta testing, answering questions, improving docs, preparing
translations and moderation all count — see the
[community guidelines](docs/community/guidelines.md#ways-to-help). How team roles work and how
to apply is described in [team roles](docs/community/team-roles.md).

## Ground rules

1. **User data comes first.** No code path may delete, overwrite or move user data without
   an explicit user decision. When in doubt, refuse and report an error.
2. **Correctness before features.** A smaller, reliable change is better than a large one.
3. **Privacy.** No telemetry, analytics or cloud services. The update check to GitHub is the only
   network access; do not add others (see [docs/updates/README.md](docs/updates/README.md)).
4. **Tests use temporary directories only.** Never point destructive tests at real folders.

## Development setup

See [docs/development/setup.md](docs/development/setup.md). In short:

```bash
npm ci
npm run app:dev
```

## Workflow

1. Open or pick an issue and comment that you are working on it.
2. Fork, then create a branch: `feat/short-name`, `fix/short-name`, or `docs/short-name`.
3. Make focused commits using [Conventional Commits](https://www.conventionalcommits.org/)
   (`feat: add split view`, `fix(transfer): keep source on verification failure`).
4. Run all checks locally:
   ```bash
   npm run typecheck
   npm test
   cargo fmt --manifest-path src-tauri/Cargo.toml --check
   cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
   cargo test --manifest-path src-tauri/Cargo.toml
   ```
   Changes in `tools/discord-setup` or `tools/discord-notify`: run `python -m pytest -q` in
   that folder.
5. Add a line under `## [Unreleased]` in `CHANGELOG.md` for user-visible changes.
6. Open a pull request and fill in the template.

## Coding conventions

- **Rust** (`src-tauri/`): `rustfmt` defaults (max width 140, see `rustfmt.toml`), no `unwrap()` on
  user-controlled data, all IPC input validated in `security/`, errors returned as `AppError`.
  Filesystem logic belongs in `filesystem/` or `operations/`, never in `commands/`.
- **TypeScript/React** (`src/`): strict TypeScript, function components and hooks, no direct
  `invoke()` calls outside `src/services/api.ts`, styling via design tokens in
  `src/styles/tokens.css` only.
- **New IPC command:** implement it in the domain module, add a thin wrapper in
  `commands/mod.rs`, register it in `lib.rs`, `build.rs` and `capabilities/main.json`, and add
  a typed wrapper in `api.ts`.
- **UI text** is English, short and friendly. Avoid jargon in error messages.

## Review process

- Every PR needs a passing CI run and at least one approving review from a maintainer.
  CI for first-time contributors starts after a maintainer approves the run.
- Changes to `operations/`, `security/` or `platform/` require extra scrutiny and tests that
  demonstrate safe behaviour (conflicts, links, cancellation, errors).
- Changes to `.github/workflows/` are reviewed by the project owner; actions are pinned to
  commit SHAs and workflows get the smallest possible permissions.
- Maintainers may ask for changes or split large PRs. Please be patient and kind.
- A maintainer may add `discord: announce` to a merged PR to highlight it on Discord.

Maintainers label new issues within a few days (`needs triage` → area and type labels). The
label set is defined in [`.github/labels.json`](.github/labels.json).

## Releases

Releases are made by the project owner; see [docs/release/process.md](docs/release/process.md).

## Community

Questions about contributing are welcome in GitHub issues, and in the Nexdir Discord
(`#contribution-help`) once its invite link is published in the README.
Where to ask what: [SUPPORT.md](SUPPORT.md).
