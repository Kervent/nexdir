# Nexdir 0.5.0-alpha.1 (alpha pre-release)

> [!WARNING]
> **This is an alpha version for early testers.** Keep backups of important files. The release
> files are **not code-signed** for Windows: SmartScreen may warn. Check the SHA-256 checksum
> before running a file.

## What's new

- **Updates from inside Nexdir.** *Help → Check for Updates* shows whether a newer version
  exists, with its release notes, date and size. Nothing is downloaded until you click
  *Download Update*, and nothing is installed until you confirm *Close Nexdir and Install*.
  Nexdir then restarts in the new version and keeps all your settings.
- **Signed updates.** Every update is signed with the Nexdir update key. Nexdir verifies the
  signature — and that it was made for exactly the announced version — before installing, and
  discards anything that does not match. It never installs an older version.
- **Stable and Beta (Preview) channels.** Alpha and beta versions come through *Beta
  (Preview)*; *Stable* will carry finished releases from 1.0 on (it has none yet and says so).
- **Automatic checks — off by default.** Turn them on under *Settings → Updates* to check once a
  day or week; a small notice tells you about new versions. With them off, Nexdir makes no update
  requests at all.
- **Open Source Licenses and Technologies & Credits** in *About Nexdir*: all bundled components
  with their licenses, searchable and available offline.
- **Settings** (gear button): appearance and update settings in one place.
- **Closing during a copy asks first**, so a running copy or move is not interrupted by accident.

## Updating from 0.4.3

0.4.3 has no updater. Download and run the installer of the same type you used before
(`Setup.exe` or `.msi`); your settings, pinned folders and search index are kept. From 0.5.0 on,
Nexdir updates itself after you confirm. The portable version is updated by replacing
`Nexdir.exe` with the one from the new ZIP.

## Privacy

Nexdir still has no telemetry and no accounts. The only connection it makes itself is the update
check to GitHub (when you click *Check for Updates* or turned on automatic checks). It sends no
file names, paths or personal data; like any web request it reveals your IP address to GitHub.
Details: [docs/updates/README.md](https://github.com/Kervent/nexdir/blob/main/docs/updates/README.md#privacy).

## Files

| File | Use |
|---|---|
| `Nexdir-0.5.0-alpha.1-Setup.exe` | Installer for the current user, no administrator rights. **Recommended.** |
| `Nexdir-0.5.0-alpha.1.msi` | Windows Installer package (for all users; needs administrator rights) |
| `Nexdir-0.5.0-alpha.1-Portable.zip` | Portable version, no installation |
| `*.sig` | Update signatures, used by Nexdir's updater |
| `SHA256SUMS.txt` | SHA-256 checksums of all files |

Requirements: Windows 10 or 11, 64-bit, with the Microsoft Edge WebView2 Runtime (included in
Windows 11; the installers add it if missing).

Full list of changes: [CHANGELOG.md](https://github.com/Kervent/nexdir/blob/main/CHANGELOG.md).
