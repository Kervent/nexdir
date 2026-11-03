## Summary

<!-- What does this change and why? Link the issue: "Closes #123". -->

## Type of change

- [ ] Bug fix
- [ ] New feature or improvement
- [ ] Documentation
- [ ] Build, CI or tooling (`tools/`, `.github/`)

## How was this tested?

<!-- Tick what you ran. Describe manual tests below: what you did and on which Windows version. -->

- [ ] `npm run typecheck`
- [ ] `npm test`
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (see CONTRIBUTING.md)
- [ ] Tool tests (`python -m pytest` in `tools/discord-setup` or `tools/discord-notify`)
- [ ] Manually tested in the running app:

## Safety checklist

- [ ] No operation can delete, overwrite or move user data without an explicit user decision.
- [ ] Destructive tests only use temporary directories; no personal files were used as test data.
- [ ] New IPC commands validate their input in Rust and are listed in `build.rs` and `capabilities/main.json`.
- [ ] No telemetry, tracking or network access was added (the update check is the only allowed connection).
- [ ] No secrets, tokens or webhook URLs are included.

## Docs

- [ ] User-visible changes are listed under `## [Unreleased]` in `CHANGELOG.md`.
- [ ] Docs were updated, or no update is needed.

## Screenshots (UI changes)
