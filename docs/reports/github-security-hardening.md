# GitHub security hardening — Kervent/nexdir

Date: 2026-11-03. Starting point: [github-security-audit.md](github-security-audit.md). Settings
were changed through the GitHub REST API with the owner's account; workflow and policy changes went
through pull request #2. Status: **PASS** (applied and verified), **FAIL**, **SKIPPED** (decided
against, with reason), **NOT TESTED**, **PENDING** (waits for a step or approval).

## Before / after

| Setting | Before | After | Status |
|---|---|---|---|
| Ruleset `Protect main` (default branch) | none | pull request required (0 approvals), review threads resolved, 7 required CI checks bound to GitHub Actions (app 15368), linear history, no force push, no deletion, merge methods squash/rebase, **no bypass actors** (admins included) | PASS |
| Ruleset `Protect update-feed` | none | no force push, no deletion (normal pushes by the update-feed workflow stay possible) | PASS |
| Ruleset `Protect release tags` (`refs/tags/v*`) | none | no deletion, no update (tags cannot be moved), no non-fast-forward; creating new tags allowed | PASS |
| Merge methods | merge/squash/rebase | squash/rebase only, branches deleted after merge, "update branch" button | PASS |
| Dependabot security updates | off | on (pull requests only, no auto-merge) | PASS |
| Dependabot version updates | npm/cargo weekly, actions/pip monthly | npm, cargo, actions weekly (Monday) and grouped; pip monthly; open-PR limits | PASS (PR #2) |
| Actions allowed | all | GitHub-owned + `Swatinem/rust-cache@*` + `softprops/action-gh-release@*` | PASS |
| SHA pinning of actions | not enforced | **required** | PASS |
| Fork PR workflow approval | first-time contributors | all external contributors | PASS |
| CodeQL | not configured | default setup (actions, JavaScript/TypeScript, Python, Rust), on PRs, pushes to `main`, weekly | PASS |
| Environment `release` | — | required reviewer Kervent (self-review allowed), deployments only from `main` and `v*` tags; holds both signing secrets | PASS |
| Release / verify workflows | repository secrets, any branch | `environment: release`; verify runs only on `main` or manually | PASS (PR #2, merged as `36b31ce`) |
| Repository-level signing secrets | present | **deleted** (owner's OK) before the proof run, so only the environment secrets exist | PASS |
| `SECURITY.md` | reporting channel, short policy | + what to include, handling process, coordinated disclosure, supported-versions table | PASS (PR #2) |
| Dependabot alert #1 (glib) | open | dismissed (not used), reason recorded | PASS |
| CodeQL alert #1 | — | fixed in code (PR #2, `5834c08`) | PASS (closes with the next analysis of `main`) |
| CodeQL alerts #2–#5 | — | open; proposed dismissals need the owner's OK | PENDING |

## Decisions

- **Required reviews: 0 approvals.** With one maintainer, a required second approval would block
  every own pull request. The pull-request rule still forces every change through CI and through
  a reviewable PR; review threads must be resolved.
- **Required checks** are the seven CI job names exactly as GitHub reports them, bound to the
  GitHub Actions app so no other integration can set them. "Dependency review" only runs on pull
  requests, which is where the rule applies. Strict "up to date with main" is off to avoid
  needless re-runs of Dependabot PRs; linear history keeps the history readable. CodeQL is not a
  required check yet (its first Rust analysis takes long; it can be added as a `code_scanning`
  rule once it has run reliably on a few PRs).
- **No bypass actors.** Emergency path: the admin can edit or disable the ruleset; every such
  change is recorded in the repository's audit trail.
- **Signed commits: SKIPPED.** No GPG/SSH signing key is set up for the maintainer's Git client;
  requiring signatures now would block all pushes. Recommended next step once a signing key is
  configured and tested.
- **Restricting pushes to `update-feed` to GitHub Actions only: SKIPPED for now.** A bypass for the
  Actions app would be needed and can only be verified by a real feed change (next release).
  Force push and deletion are already blocked.
- **Secret scanning extras** (non-provider patterns, validity checks): NOT AVAILABLE on this plan.
- **Artifact attestations** (`actions/attest-build-provenance`): possible for public repositories
  and complementary to the Tauri update signatures (it proves *which workflow run* built a file;
  it does not replace signature verification in the updater). Not added yet because the release
  workflow cannot be test-run without creating a release; recommended for the next release, then
  verifiable with `gh attestation verify`.

## Verification

| Test | Result |
|---|---|
| Effective rules for `main` via `GET /rules/branches/main` | deletion, non_fast_forward, required_linear_history, pull_request, required_status_checks — PASS |
| Effective rules for `update-feed` | deletion, non_fast_forward — PASS |
| **Direct push** rejected — against a temporary branch `ruleset-test/temp` with a temporary copy of the pull-request and status-check rules (so `main` was never at risk) | `GH013 … Changes must be made through a pull request … 7 of 7 required status checks are expected`, branch unchanged — PASS. Temporary ruleset and branch deleted afterwards. |
| Force push / deletion protection | verified through the API only (rules present); **no force push was attempted**, as instructed — NOT TESTED by action |
| Normal PR workflow under the new rules | PR #2: all 7 required checks green, status CLEAN, merged by rebase through the ruleset (no bypass) — PASS |
| `release` environment secrets present (names only) | `TAURI_SIGNING_PRIVATE_KEY`, `…_PASSWORD` set 16:41 UTC; password checked locally against the key before upload — PASS |
| Signing through the environment | run [41679019312](https://github.com/Kervent/nexdir/actions/runs/41679019312) on `main` waited for approval (`waiting`), was approved by Kervent for `release`, signed and verified a test file with the **environment** secrets (repository secrets already deleted) — PASS |
| Update-feed workflow under the new rules | Not affected by `Protect main`; `update-feed` rules only block force push/deletion. NOT TESTED by a real run (no new release) |
| Release workflow | PENDING with the signing check; a full run needs a new tag (not created) — NOT TESTED |

## Dependabot pull requests and the license check

The first Dependabot pull request (#3, `lucide-react` 1.65.0 → 1.66.0, group `npm-minor`) ran
all required checks: everything passed except **Security**, because `THIRD-PARTY-NOTICES.txt`
and the in-app license index no longer match the new version. This is intended — no dependency
change ships without updated license information — but it means every Dependabot PR needs one
manual step before it can be merged:

```bash
gh pr checkout <number>
python tools/release/third_party_notices.py      # regenerates the notices and the license index
git commit -am "Update third-party notices"
git push
```

After the push the checks run again; review the license changes in the diff, then merge. No
Dependabot PR is merged automatically.

## Remaining risks

- The maintainer account is the single point of trust (admin, sole approver of the release
  environment, holder of the key backup). Two-factor authentication on the account is essential
  (cannot be verified through the repository API).
- CodeQL alerts #2–#5 stay open until the owner decides.
- No Authenticode signing of the Windows binaries.
