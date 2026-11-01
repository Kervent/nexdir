# Nexdir 0.4.3 (alpha pre-release)

> [!WARNING]
> **This is an alpha version for early testers.** Keep backups of important files before you use
> Nexdir on them, and report anything unexpected. The release files are **not code-signed**:
> Windows SmartScreen may warn. Check the SHA-256 checksum before running a file (see below).

Nexdir is a modern, open-source file manager for Windows: tabs, split view, safe copy and move
with conflict handling, plain-English search, storage analysis, a duplicate finder and
organisation rules. No telemetry, no accounts, no cloud. This is the first public release.

## What's new in 0.4.3

- **Welcome tour.** On the first start Nexdir offers a one-minute tour of the real interface:
  navigation, tabs and split view, safe file operations, smart tools and personalization. You can
  skip it with one click and restart it any time with *Help → Take a Tour*.
- **Help & Support.** A new question-mark button opens the Help menu. *Help & Support* has offline
  help for navigation, file operations, search, duplicates, storage analysis, organize rules,
  keyboard shortcuts and common problems, plus links to the documentation and this repository.
- **Report a bug from the app.** *Help → Report a Bug* asks for a short description, then shows
  exactly what will be shared. File paths, your user name, e-mail addresses and password-like
  values are replaced with placeholders; the Nexdir and Windows version can be left out. Nexdir
  then opens the prefilled GitHub issue form in your browser; nothing is sent until you submit it.
  *Request a Feature* works the same way.
- **Safer duplicate removal.** The rule "always keep at least one copy" is now enforced by the
  backend, not only by the interface. Before anything goes to the Recycle Bin, Nexdir locks one
  kept copy and reads it again, and re-checks every copy to remove; files that changed, vanished or
  are links are skipped. Hard links are no longer reported as duplicates.

Earlier work included in this first public release (0.4.0–0.4.2, never published separately):
staged, verified replacement of existing files so a failed or cancelled copy can never destroy
the original; handle-based file operations that cannot hit a file another program swapped in;
stricter path validation; faster filtering in very large folders.
Full list: [CHANGELOG.md](https://github.com/Kervent/nexdir/blob/main/CHANGELOG.md).

## Files

| File | Use |
|---|---|
| `Nexdir-0.4.3-Setup.exe` | Installer for the current user, no administrator rights. **Recommended.** |
| `Nexdir-0.4.3.msi` | MSI package, installs for all users (needs administrator rights) |
| `Nexdir-0.4.3-Portable.zip` | Portable `Nexdir.exe` with license files — unzip and run |
| `SHA256SUMS.txt` | Checksums of the three files (also listed at the end of these notes) |

## Installation

1. Download `Nexdir-0.4.3-Setup.exe` (or the portable ZIP).
2. Check it: `Get-FileHash .\Nexdir-0.4.3-Setup.exe -Algorithm SHA256` in PowerShell must print the
   checksum listed for that file in `SHA256SUMS.txt`.
3. Run it. If SmartScreen shows "Windows protected your PC", choose *More info → Run anyway*. The
   files are unsigned because the project has no code-signing certificate yet.

Updating: run the new installer over an existing installation; settings are kept.
Uninstalling: *Settings → Apps → Nexdir*. Settings are kept unless you tick *Delete the application
data*.

## System requirements

- Windows 10 or 11, 64-bit (x64)
- Microsoft Edge WebView2 Runtime — included in Windows 11 and current Windows 10; the installer
  downloads it if missing, the portable version expects it to be present
- No administrator rights needed (except for the MSI)

## Known limitations

- Tested on Windows 11 (NTFS, SSD) only. Windows 10, FAT/exFAT and USB drives, network shares,
  high-DPI monitors and screen readers have not been tested yet.
- Nexdir never deletes permanently. On drives without a Recycle Bin (network shares, most USB
  drives) deleting is refused with a message; use Explorer there.
- Replacing files and moving between drives verify the data first and take longer than in Explorer.
- The Discord community link is not available in the app yet.
- No thumbnails in the icon view yet; see the
  [roadmap](https://github.com/Kervent/nexdir/blob/main/docs/ROADMAP.md).
- Not crash-proof: if Windows or the PC fails during a copy, a temporary file named
  `~nexdir-….partial` or `~nexdir-….backup` may remain; the original is never deleted first.

Found a problem? Use *Help → Report a Bug* in Nexdir or open an
[issue](https://github.com/Kervent/nexdir/issues/new/choose). Security issues:
[report privately](https://github.com/Kervent/nexdir/security/advisories/new).
