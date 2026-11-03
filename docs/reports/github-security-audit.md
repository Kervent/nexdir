# GitHub security audit — Kervent/nexdir

Date: 2026-11-03 · Repository: public, personal account (GitHub Free) · One collaborator:
Kervent (admin) · No deploy keys, no webhooks. All values read through the GitHub REST API (`gh`)
before any change; raw responses kept with the working notes.

Status legend: **ACTIVE** (on before this audit) · **AVAILABLE** (could be enabled) ·
**NOT CONFIGURED** · **NOT AVAILABLE** (needs a paid plan or not applicable).

## State before hardening

| Area | State | Notes |
|---|---|---|
| Branch protection / rulesets | NOT CONFIGURED | `main` unprotected (`404 Branch not protected`), no rulesets, tags unprotected |
| Merge methods | merge, squash, rebase all allowed | branches not deleted after merge |
| Actions enabled | ACTIVE | all actions allowed, SHA pinning not enforced (all workflows pin SHAs anyway) |
| Default `GITHUB_TOKEN` | read-only | `can_approve_pull_request_reviews: false` |
| Fork pull-request workflows | approval for first-time contributors only | |
| Dependency graph | ACTIVE since 2026-11-03 | enabled earlier the same day so Dependency review works |
| Dependabot alerts | ACTIVE since 2026-11-03 | 1 alert (see below) |
| Dependabot security updates | NOT CONFIGURED | |
| Dependabot version updates | ACTIVE (`.github/dependabot.yml`) | npm/cargo weekly, pip/actions monthly |
| Dependency review | ACTIVE (CI job on pull requests) | |
| Secret scanning | ACTIVE | 0 alerts |
| Push protection | ACTIVE | |
| Secret scanning: non-provider patterns, validity checks | NOT AVAILABLE | enabling via API had no effect (requires GitHub Secret Protection) |
| Code scanning (CodeQL) | NOT CONFIGURED | |
| Private vulnerability reporting | ACTIVE | `SECURITY.md` points to it |
| Environments | `discord` (no rules, no secrets) | |
| Secrets | `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` at **repository level** | usable by any workflow on any branch a collaborator pushes |
| Variables | none | |

## Workflows

| Workflow | Trigger | Token permissions | Secrets | Findings |
|---|---|---|---|---|
| CI | push to main, pull requests | `contents: read` | none | fine; actions pinned to SHAs |
| Release | manual (`workflow_dispatch`) | read; job `contents: write` | signing key | refuses existing releases and mismatching tags, draft only. **Risk:** dispatchable from any branch, so a workflow file changed on a branch would receive the signing secrets |
| Update feed | release events, manual | read; job `contents: write` | none (`GITHUB_TOKEN`) | checks out `main`, writes only to branch `update-feed`; tag name passed through environment variables (no script injection) |
| Verify update signing key | manual, push of its own file | `contents: read` | signing key | **Risk:** push trigger on every branch |
| Discord announcements | release, labels, `pull_request_target` (closed/labeled), milestones | `{}`; job `contents: read` | webhooks in env `discord` (none set) | `pull_request_target` used safely: base branch checked out, only PR metadata read, runs only for merged PRs with a label only maintainers can set |
| Sync labels | manual | job `issues: write` | none | fine |

All third-party actions are pinned to full commit SHAs. External (non-GitHub) actions in use:
`Swatinem/rust-cache`, `softprops/action-gh-release`. No `${{ }}` expressions inside `run:`
scripts (checked by the notifier's workflow tests). Artifacts: only the 7-day CI build of
`nexdir.exe` and the notifier's dry-run output.

## Alerts found

| Source | Alert | Assessment | Action |
|---|---|---|---|
| Dependabot #1 | `glib` 0.18.5, GHSA-wrw7-89jp-8q8g (medium, unsoundness) | Linux-only dependency of Tauri's GTK stack; `cargo tree --target x86_64-pc-windows-msvc -i glib` prints nothing; not shipped | dismissed as *vulnerable code not used* (owner's approval), with reason |
| CodeQL #1 | `js/incomplete-multi-character-sanitization`, `ReleaseNotes.tsx` | Not exploitable (React renders text, never HTML), but nested tags could leave visible remnants | **fixed** in PR #2 with a regression test (closed by CodeQL as fixed) |
| CodeQL #2 | same rule, `tools/branding/build-branding.mjs` | build-time tool, input is the project's own SVG | dismissed as *false positive* (owner's OK), reason recorded |
| CodeQL #3, #4 | `py/clear-text-logging-sensitive-data`, `tools/release/update_feed.py` | logs only tags, file names, verification errors and the *public* signed comment; the token is never printed | dismissed as *false positive* (owner's OK), reasons recorded |
| CodeQL #5 | `py/incomplete-url-substring-sanitization`, notifier test `conftest.py` | fake HTTP router in tests | dismissed as *used in tests* (owner's OK), reason recorded |
| CodeQL #6 | same rule as #1, on the first fix in `ReleaseNotes.tsx` | the regex variants were not recognised as complete by CodeQL | **fixed** in PR #5 with a character filter (`stripTags`) and unit tests; closed by CodeQL as fixed |
| Secret scanning | 0 alerts | — | — |

## Secret scan of the Git history

Independent of GitHub's secret scanning, every commit reachable from any branch or tag
(`ee01c35`, `a183242`, `d73e315`, `d276c07`, `4148c1e`, `ccc93d3`) and the `update-feed` branch
(`c31f8fc`) was scanned with Yelp **detect-secrets 1.5.0** (isolated virtual environment). The
pre-rewrite commit `fc28947` (still reachable by SHA on GitHub) has the same file tree as
`ee01c35`. 7 distinct hits, all false positives, none printed in full:

| File | Type | Why it is not a secret |
|---|---|---|
| `src-tauri/tauri.conf.json` | Base64 high entropy | the update **public** key |
| `preview.json` (update-feed) ×2 | Base64 high entropy | public update signatures |
| `src-tauri/src/updater/feed.rs` | Base64 high entropy | test constant decoding to "untrusted comment: signature from tauri secret key" |
| `src-tauri/windows-app-manifest.xml` | Hex high entropy | Microsoft's public token for Common Controls |
| `src/features/help/report.test.ts` | Base64 / Private key | privacy-filter test data assembled at run time (`0123…abc…`, key body `abc`) |

No credential needs rotating.
