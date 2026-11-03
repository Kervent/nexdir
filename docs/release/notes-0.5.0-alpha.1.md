# Nexdir 0.5.0-alpha.1 (alpha pre-release)

> [!WARNING]
> **This is an alpha version for early testers.** Keep backups of important files. The release
> files are **not code-signed** for Windows: SmartScreen may warn. Check the SHA-256 checksum
> before running a file.

## What's new

- **Secure, signed update system.** *Help → Check for Updates* (also in *About Nexdir* and
  *Settings → Updates*) asks GitHub whether a newer version exists and shows its release notes,
  date and download size. Nothing is downloaded until you click *Download Update*, and nothing is
  installed until you confirm *Close Nexdir and Install*. Every update must be signed with the
  Nexdir update key **for exactly the announced version**; anything else is discarded. Nexdir never
  installs an older version.
- **Download progress.** The update window shows real progress, speed and the signature status,
  and explains problems (offline, GitHub not reachable, invalid signature, not enough disk space)
  in plain words.
- **Stable and Beta (Preview) channels.** Alpha and beta versions come through *Beta (Preview)*,
  which is where this version starts. *Stable* will carry finished releases; it has none yet and
  says so instead of offering anything.
- **Optional automatic checks — off by default.** Turn them on under *Settings → Updates* to
  check at most once a day or week. A small, non-blocking notice offers *View Changes*,
  *Download Update* and *Remind Me Later*. Automatic checks never download or install anything.
  With them off, Nexdir makes no update requests at all.
- **Open Source Licenses.** *About Nexdir → Open Source Licenses* lists all 234 bundled components
  with version, license, project page and full license text — searchable, filterable and
  available offline. The same information is in `THIRD-PARTY-NOTICES.txt`.
- **Technologies & Credits** in *About Nexdir*, plus version, release channel and build information.
- **Settings dialog** (gear button) for appearance and updates.
- **Safety improvements.** Closing Nexdir while a copy or move is running now asks first;
  installing an update is refused while file operations run; Escape reliably closes dialogs.

## Updating from 0.4.3

Version 0.4.3 has **no updater**, so this version must be installed **manually** once: download
the installer of the same type you used before (`Setup.exe` or `.msi`) and run it. Your settings,
pinned folders, organize rules and search index are kept (tested with the per-user setup).

From this version on, Nexdir can install **future** signed releases itself after you confirm. The
complete path — check, download, signature check, installation, restart — was tested on Windows
with test builds and test keys; the first real use will be the next release.

## Installed and portable version

| | Installer (`Setup.exe`) | MSI package | Portable ZIP |
|---|---|---|---|
| Installs for | the current user, no administrator rights | all users, needs administrator rights | — (unpack anywhere) |
| Future updates | in the app, after confirmation | in the app, after confirmation (Windows may ask for administrator permission) | announced in the app; replace `Nexdir.exe` yourself |

The portable version never changes itself. Settings of all versions live in your user profile
(`%APPDATA%\org.nexdir.desktop`, `%LOCALAPPDATA%\org.nexdir.desktop`).

## How downloads are protected

- **Update signatures (`.sig` files):** Nexdir's updater checks each installer against the public
  Nexdir update key built into the app, and checks that the signature was made for this exact
  version. Unsigned or modified installers are never installed by the updater.
- **SHA-256 checksums (`SHA256SUMS.txt`):** compare them when you download a file by hand:
  `Get-FileHash .\Nexdir-0.5.0-alpha.1-Setup.exe -Algorithm SHA256`.
- **Windows code signing (Authenticode):** not yet. That is why SmartScreen may show
  *Windows protected your PC*; choose *More info → Run anyway* only after checking the checksum.
  Update signatures and checksums do not replace code signing.

## Privacy

Nexdir still has no telemetry and no accounts. The only connection it makes itself is the update
check to GitHub (when you click *Check for Updates* or turned on automatic checks). It sends no
file names, paths or personal data; like any web request it reveals your IP address to GitHub.
Details: [docs/updates/README.md](https://github.com/Kervent/nexdir/blob/main/docs/updates/README.md#privacy).

## Data safety

Nexdir never deletes files permanently (removed items go to the Recycle Bin) and never replaces a
file without asking. This is still alpha software: **keep backups** of important files, and of
`%APPDATA%\org.nexdir.desktop` if your settings matter to you.

## Known limitations

- Alpha quality; tested on Windows 11 only. Windows 10 is supported but not yet tested.
- The in-app update of an **MSI** installation has not been tested on a real machine yet (the
  per-user setup has).
- The update feeds are created when this release is published; caches can delay a new version by
  a few minutes.
- No Windows code signing yet (see above). The installers grew by about 1 MB because of the secure
  HTTPS support for update checks.
- A copy that was just cancelled may still be finishing for a moment; *Install* asks you to wait
  until it has.

## Files

| File | Use |
|---|---|
| `Nexdir-0.5.0-alpha.1-Setup.exe` | Installer for the current user, no administrator rights. **Recommended.** |
| `Nexdir-0.5.0-alpha.1.msi` | Windows Installer package for all users (administrator rights) |
| `Nexdir-0.5.0-alpha.1-Portable.zip` | Portable version, no installation (includes `LICENSE.txt` and `THIRD-PARTY-NOTICES.txt`) |
| `Nexdir-0.5.0-alpha.1-Setup.exe.sig`, `Nexdir-0.5.0-alpha.1.msi.sig` | Update signatures, used by Nexdir's updater |
| `SHA256SUMS.txt` | SHA-256 checksums of the files above |

**Requirements:** Windows 10 or 11, 64-bit, with the Microsoft Edge WebView2 Runtime (part of
Windows 11; the installers add it if missing; the portable version needs it installed).

License: MIT. Full list of changes: [CHANGELOG.md](https://github.com/Kervent/nexdir/blob/main/CHANGELOG.md).
