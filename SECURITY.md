# Security Policy

Nexdir handles personal files, so we take security seriously.

## Reporting a vulnerability

**Please do not open public issues or Discord posts for security problems.**

Report vulnerabilities privately via a
[GitHub security advisory](https://github.com/Kervent/nexdir/security/advisories/new)
("Report a vulnerability" on the repository's *Security* tab, see
[how it works](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)).

Please include the Nexdir version, Windows version, steps to reproduce and the impact you
expect. We aim to acknowledge reports within 7 days. We will credit reporters in the
release notes unless you prefer otherwise.

## Supported versions

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
- No telemetry, no network access, no auto-updater.

Details: [docs/security/threat-model.md](docs/security/threat-model.md).
