# Team roles

Nexdir separates three things that are easy to mix up:

1. **Project roles** — what someone does for Nexdir (develop, review, support, moderate).
2. **GitHub repository access** — who can merge, label or release.
3. **Discord server permissions** — who can manage the server.

Having one never automatically grants another. In particular, a project role never comes
with server administration, and **write access to the repository is granted only by the
project owner**.

## Roles and responsibilities

| Role | Responsibilities | GitHub access | Discord role |
|---|---|---|---|
| **Project owner** (Founder) | Vision, final decisions, releases, code signing, granting access, security advisories | Admin (owner) | Founder |
| **Core Developer** | Technical direction, reviews of risky areas (`operations/`, `security/`, `platform/`), release preparation, triage | Maintain or Write — granted by the owner | Core Developer |
| **Developer** | Regular code contributions and reviews | Usually none (works from a fork); Triage if they help with issues | Developer |
| **Support Team** | Helps users in #support and #bug-reports, reproduces bugs, turns confirmed bugs into GitHub issues | Triage (label, close, reopen) — optional | Support Team |
| **Moderator** | Enforces the Code of Conduct on Discord, handles reports, spam and raids | None needed | Moderator |
| **Administrator** | Trusted server administration on Discord (channels, roles below their own) | None needed | Administrator |
| **Contributor** | Has at least one merged contribution (code, docs, translation, design) | None | Contributor |
| **Beta Tester** | Tests pre-releases and reports structured feedback | None | Beta Tester |
| **Member** | Everyone in the community | — | Member |

GitHub permission levels are explained in the
[GitHub documentation](https://docs.github.com/en/organizations/managing-user-access-to-your-organizations-repositories/managing-repository-roles/repository-roles-for-an-organization).
"Triage" lets someone label and close issues without being able to push code.

### Rules that always apply

- **Nobody gets the Discord *Administrator* permission flag** through a role. The Founder role
  has broad but explicit permissions; only the server owner holds full ownership.
- **Branch protection on `main`**: changes arrive through pull requests with passing CI and an
  approving review — also for maintainers. Recommended settings are in
  [docs/release/process.md](../release/process.md#repository-settings).
- Releases are published only by the project owner (or someone they explicitly delegate a
  specific release to).
- Access follows the work: inactive team roles are removed after 90 days of inactivity, after a
  friendly check-in. Returning is always welcome.
- Every role can be stepped down from at any time — just tell a moderator or the owner.

## Becoming part of the team

There is no form to fill out on GitHub and no automatic promotion. The path is:

1. **Contribute in the open** for a while — pull requests, reviews, answers in #support,
   well-written bug reports, tested pre-releases.
2. **Apply** by sending a direct message to a moderator or administrator on Discord with:
   ```text
   Position:
   Discord name:
   GitHub profile (for development roles):
   Time per week:
   Relevant experience:
   Why do you want to help?
   Languages spoken:
   ```
3. **Review** happens in the private #staff-applications channel:
   - the owner or an administrator reviews every application;
   - Core Developers also review development roles, Moderators also review moderation and support roles;
   - development roles require at least one merged pull request or substantial review work first.
4. **Decision within 14 days.** Applicants get a friendly answer either way. If accepted, the role
   is assigned **by hand**; GitHub access (if any) is granted separately by the owner.

Applications are never accepted automatically, and the Discord setup tool cannot assign team
roles.

## Contribution areas without a team role

You don't need a role to help. See [guidelines.md](guidelines.md#ways-to-help) for development,
beta testing, community support, documentation, translation and moderation help. The
"Interested: …" roles on Discord only show what you'd like to help with; they carry no
permissions.

## Where the role setup is defined

- Discord roles and channel access: [`tools/discord-setup/config/server.toml`](../../tools/discord-setup/config/server.toml),
  summarised in [discord.md](discord.md).
- Labels used for triage: [`.github/labels.json`](../../.github/labels.json).
