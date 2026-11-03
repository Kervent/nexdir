# Security Policy

Nexdir handles personal files, so we take security seriously.

## Reporting a vulnerability

**Please do not open public issues or Discord posts for security problems.**

Report vulnerabilities privately via a
[GitHub security advisory](https://github.com/Kervent/nexdir/security/advisories/new)
("Report a vulnerability" on the repository's *Security* tab, see
[how it works](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)).

Private vulnerability reporting is the only channel for security reports; there is no
security e-mail address.

### What to include

- Nexdir version (*About Nexdir*), installation type (setup, MSI, portable) and Windows version
- What an attacker can do, and what they need for it (a crafted file or folder name, a malicious
  network, a modified update feed, local access, …)
- Steps to reproduce, ideally with a minimal example — never real personal files
- Your assessment of the impact, and whether you have seen it exploited

### How reports are handled

1. Acknowledgement within 7 days.
2. Assessment and, if confirmed, a fix developed in a private fork of the advisory.
3. A fixed release, signed for the in-app updater, published as soon as it is ready.
4. A published GitHub security advisory (with a CVE where appropriate) describing the problem
   and the fixed version. Reporters are credited unless they prefer otherwise.

Please give us the chance to ship a fix before disclosing details publicly. Nexdir works on
people's personal files and installs updates; publishing a vulnerability before a fixed version
is available would expose users who cannot protect themselves yet. If a report affects the
update signing key, we treat the key as compromised and follow the rotation procedure in
[docs/updates/signing.md](docs/updates/signing.md).

## Supported versions

| Version | Security fixes |
|---|---|
| Latest 0.5.x pre-release | Yes |
| Older 0.5.x pre-releases | No — update in the app (*Help → Check for Updates*) |
| 0.4.3 and older | No — no updater; install the latest version manually |

Until 1.0, only the latest release receives security fixes.

## Security model (summary)

- The WebView has no direct filesystem or shell access. All file operations go through a
  fixed set of typed Rust commands, each enabled explicitly in
  `src-tauri/capabilities/main.json`.
- Every path and name from the UI is validated in Rust (`src-tauri/src/security`):
  absolute paths only, `..` segments and device paths rejected, Windows-reserved names
  rejected.
- Nothing is deleted permanently. Removal goes through the Windows Recycle Bin, and on
  drives without a Recycle Bin Nexdir refuses.
- Existing files are never replaced without an explicit choice; renames and moves use
  atomic "fail if exists" Win32 calls to avoid check-then-act races.
- Copies go to an exclusively created temporary file, are verified (BLAKE3) and only then put
  in place; a replaced file is kept as a backup until the new one is in place. Temporary files,
  backups and move sources are renamed and deleted only through Nexdir's own file handles, never
  by path ([details](docs/security/safe-copy.md)).
- Recursive copies never follow symbolic links or junctions.
- Programs, scripts and shortcuts only run after a confirmation dialog, enforced in the
  backend as well.
- No telemetry. The only network access is the update check to GitHub (manual, or automatic if
  enabled; off by default). Updates are downloaded and installed only after the user confirms,
  and only if their minisign signature from the Nexdir update key — bound to the announced
  version — verifies ([docs/updates/signing.md](docs/updates/signing.md)). Downgrades are
  refused.

Details: [docs/security/threat-model.md](docs/security/threat-model.md).
