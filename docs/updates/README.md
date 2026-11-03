# Updates

From version 0.5.0 Nexdir can find, download and install new versions itself. It never does so
without you: nothing is downloaded until you click **Download Update**, and nothing is installed
until you confirm **Close Nexdir and Install**.

## Checking for updates

- **Help → Check for Updates** (the question-mark button), **About Nexdir → Check for Updates**,
  or **Settings → Updates → Check for Updates**. A manual check works at any time.
- **Automatic checks** are **off** by default. Turn them on under *Settings → Updates →
  Automatically check for updates*. Nexdir then checks about 8 seconds after it starts, at most
  once a day (or once a week, as you choose). It keeps no connection open and never checks twice
  at the same time. A failed check (for example offline) does not disturb you and is not retried
  before the interval has passed.
- When an automatic check finds an update, a small notice appears at the bottom left —
  *View Changes*, *Download Update* or *Remind Me Later* (no reminder for that version for a
  day). Turn the notice off with *Notify me when an update is available*.

## The update window

Shows the installed and the available version, the release date, the channel, the download size,
the signature status and the release notes. While downloading it shows real progress and speed
from the updater; you can cancel or hide the window (the download continues). After the download
the signature is verified and the window says *ready to install*. Installing closes Nexdir, runs
the installer with a progress window, and starts Nexdir again. The next start tells you whether
the update was installed.

Nexdir refuses to install while a copy or move is running or the search index is being built:
finish or cancel those first. Closing Nexdir while file operations run also asks first.

## Release channels

| Channel | Offers | For |
|---|---|---|
| **Stable** | Finished releases only (no alpha/beta, not marked pre-release on GitHub) | Everyone, once 1.0 is out |
| **Beta (Preview)** | Alpha and beta releases, and stable releases when they are newer | Testers; expect bugs and keep backups |

- Nexdir is in alpha, so **the Stable channel has no release yet** and says so — it never shows
  an invented version.
- A new installation follows the channel of its own version: alpha/beta builds start on
  *Beta (Preview)*, stable builds on *Stable*. Your choice is kept.
- Switching channels **never installs an older version**. Switching from Beta to Stable while
  running 0.6.0-beta keeps 0.6.0-beta until a newer stable release exists.
- Versions are compared by [Semantic Versioning](https://semver.org): `0.5.0-alpha.2` <
  `0.5.0-beta.1` < `0.5.0-rc.1` < `0.5.0` < `0.5.1`.

## Privacy

Nexdir has no telemetry and no accounts. With automatic checks off, Nexdir makes **no** update
requests. A check (manual or automatic) does this:

1. One HTTPS request for a small JSON file, `stable.json` or `preview.json`, from
   `raw.githubusercontent.com/Kervent/nexdir/update-feed/`.
2. Only if you click *Download Update*: one HTTPS download of the installer from
   `github.com/Kervent/nexdir/releases/download/…` (GitHub redirects to its file servers).

What is transmitted: what every web request transmits — your IP address, the time, and an HTTP
header naming the program (`Nexdir/0.5.0-alpha.1`, and `tauri-plugin-updater/…` for downloads).
GitHub (Microsoft) receives this as the operator of those servers; see the
[GitHub privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
What is **not** transmitted: file names, folder contents, paths, personal files, e-mail
addresses, device or hardware IDs, settings, usage data. No cookies, no account.

## Installation types

| Installed with | How updates are installed |
|---|---|
| `Nexdir-<v>-Setup.exe` (per user) | The new per-user setup runs in passive mode (progress window, no questions) and restarts Nexdir. No administrator rights. |
| `Nexdir-<v>.msi` | The new MSI package runs through Windows Installer (`msiexec /passive`). Windows may ask for administrator permission. |
| Portable ZIP | **Never changed automatically.** Nexdir only tells you about the new version and opens the download page. Close Nexdir, replace `Nexdir.exe` with the one from the new ZIP. Settings are kept (they live in `%APPDATA%`). |

The installed type is recorded in the program file by the installer build, so an MSI
installation always receives the MSI package and a per-user installation the setup — never the
other way round. The update file is checked for the right type again before it is started.

**From 0.4.3:** version 0.4.3 has no updater. Download 0.5.0 once from the
[releases page](https://github.com/Kervent/nexdir/releases) and install it over 0.4.3 with the
same installer type you used before. Settings, pinned folders, tabs and the search index are kept.

## When an update fails

| Message | What happened | What to do |
|---|---|---|
| No connection to GitHub / did not answer in time | Offline, proxy or firewall | Check the connection, try again later |
| GitHub is limiting requests (HTTP 403/429) | Too many requests from your network | Try again later |
| No update information was found (HTTP 404) | The feed is not available | Try again later; the releases page always works |
| The update information was rejected / is malformed | The feed failed a safety check (wrong address, wrong channel, invalid version) | Nothing is downloaded; report it if it persists |
| The update's signature is invalid / does not match | The download was damaged or tampered with | It was discarded; nothing was installed. Try again later |
| The download failed / connection was interrupted | Network problem during download | *Download Update* starts again |
| Not enough free disk space | The temporary folder's drive is full | Free some space |
| A copy or move is still running | Installing would interrupt it | Wait or cancel, then install |
| The update to X was not installed (next start) | The installer was cancelled (e.g. at the administrator prompt) or failed | Your current version keeps working; try again |

A failed or cancelled installer leaves the current version in place; settings are never
touched by the update. If anything goes wrong you can always install the new version manually
from the releases page.

## For maintainers

- [Signing updates](signing.md) — keys, secrets, backup, rotation; what update signatures,
  Authenticode and checksums each do.
- [Update feeds](feeds.md) — how releases reach users and stay in sync with GitHub.
- [Testing the updater](testing.md) — local test builds with test keys.
- [Release process](../release/process.md).
