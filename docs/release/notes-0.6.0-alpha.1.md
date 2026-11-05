# Nexdir 0.6.0-alpha.1 — Network shares and NAS (alpha pre-release)

> [!WARNING]
> **This is an alpha version for early testers.** Keep backups of important files, including
> files on your NAS. The release files are **not code-signed** for Windows: SmartScreen may warn.
> Check the SHA-256 checksum before running a file.

## What's new

- **Network shares and NAS (SMB).** A new **Network** area in the sidebar shows the network
  drives and share connections Windows already has (each share once, also when it has a drive
  letter), your saved shares (also while the NAS is off) and, after you open `\\server`, the shares
  of that server. Network folders open like any other folder, in **tabs** and in **split view**
  (for example: PC on the left, NAS on the right).
- **Connect a share by its path**, for example `\\192.168.178.10\Media`, `\\NAS\Backups`,
  `//nas/share` or `smb://nas.local/share`. Nexdir connects with your Windows account; if the share
  needs a different user name and password, **Windows' own sign-in dialog** opens. Nexdir never
  sees, stores or logs passwords. Windows can remember them in its Credential Manager if you tick
  that option in Windows' dialog.
- **Saved shares** (path and name only) with rename, status check, *Open in new tab*, *Open in
  other pane*, *Copy network path* and *Properties*.
- **Search the network** on request, with progress and Stop. It lists the servers Windows reports
  and the servers you already use, and asks them for their shares. Nexdir never scans address
  ranges or ports and never searches in the background.
- **Safe copies to and from shares.** Each file is written under a temporary name, verified (size
  and BLAKE3 hash), flushed to the server and only then renamed into place. An existing file is
  never overwritten without your choice (*Skip*, *Keep both*, *Replace*). On slow networks progress
  and speed show the real transfer, and **Cancel** stops a copy within a fraction of a second.
- **Clear messages** for *Sign-in required*, *Access denied*, *Not reachable*, *Other account in
  use*, *Blocked by Windows* and *Connection lost*. A NAS that does not answer cannot freeze
  Nexdir; other tabs and the other pane keep working.
- **Help topic** *Network shares and NAS*.
- **Fixes:** notifications no longer cover the buttons of an open dialog; after cancelling
  Windows' sign-in dialog, *Sign in…* is offered again right away.

## Local and network files: what is different

| Action | Local drives | Network shares |
|---|---|---|
| Browse, preview, open, rename, new folder | ✓ | ✓ |
| Copy (both directions, also share → share) | ✓ verified | ✓ verified and flushed to the server |
| Move | ✓ | only **inside the same share** (a rename on the server). Moving between a share and another location is refused: copy instead, then delete the original yourself. |
| Delete | to the Recycle Bin | **not available**: shares have no Recycle Bin, and Nexdir never deletes permanently. Duplicate *removal* is refused on shares for the same reason. |
| Storage analysis, duplicate search, search | ✓ | ✓ (read-only) |

Nexdir does not change any Windows security setting: no SMB1, no guest access, no changes to
signing, firewall or network discovery. It releases only the network connections it created
itself when it closes; drives and connections made in Explorer or with `net use` are never
changed.

## What was tested

- **Windows 11** (Pro, build 26300). **Windows 10 is not tested yet.**
- A real **Samba 4.22** server (SMB 3.1.1 with signing) on a virtual network: connecting,
  Windows' sign-in dialog (cancel, wrong password, account without permission, correct
  sign-in, second account on the same server), file operations with SHA-256 checked on the
  server, read-only shares, drive letters, server outages during copies (no partial file under
  the final name, existing files keep their content), slow and lossy links, tabs and split view.
- The **installers of this version** (per-user setup, MSI, portable) with test builds, including
  the update from 0.5.0-alpha.1 through the in-app updater (with a test signing key).
- **Not tested:** NAS devices (Synology, QNAP, TrueNAS and others), shares of other Windows PCs
  over a physical network, server names (only IP addresses), real cable or Wi-Fi loss.

## Updating from 0.5.0-alpha.1

*Help → Check for Updates* (channel *Beta (Preview)*) offers this version once it is published.
The update is verified against the Nexdir update key and must be signed for exactly this version;
nothing is downloaded or installed without your confirmation. Your settings are kept. The portable
version announces the update; replace `Nexdir.exe` yourself.

From 0.4.3: install this version manually once (same installer type as before).

## How downloads are protected

- **Update signatures (`.sig` files):** Nexdir's updater checks each installer against the public
  Nexdir update key built into the app, and checks that the signature was made for this exact
  version. These signatures were created freshly for this release.
- **SHA-256 checksums (`SHA256SUMS.txt`):** compare them when you download a file by hand:
  `Get-FileHash .\Nexdir-0.6.0-alpha.1-Setup.exe -Algorithm SHA256`.
- **Windows code signing (Authenticode):** not yet. Update signatures and checksums are a
  separate mechanism and do not replace it; SmartScreen may show *Windows protected your PC*.
  Choose *More info → Run anyway* only after checking the checksum.

## Privacy

No telemetry, no accounts. Opening the Network area reads only what Windows already knows.
Nexdir contacts a server only when you open a share, check a connection, list a server's shares
or search the network. Saved shares are stored on your PC (path and name only). The update check
is the only connection to the internet Nexdir makes itself.

## Known limitations

- Alpha quality. Keep **backups**, also of the data on your NAS.
- Windows' network list is often empty on current Windows versions, so *Search the network* may
  only find servers you already use. **Adding a share by its path always works.**
- On networks with high latency, copies to a share are slower than in Explorer, because every
  part is confirmed by the server before the next one is sent (for example 20 MiB at 40 ms delay:
  8.1 s vs 1.8 s in our test). On bandwidth-limited links the speed is the same.
- If the server disappears during a copy, a temporary file named `~nexdir-….partial` can remain
  on the share. The error message names it; delete it once the share is back.
- Drive letters cannot be assigned from Nexdir (use Explorer or `net use`); existing drive letters
  are shown and used. Moves between a drive letter and the `\\server\share` path of the same share
  are refused (copy instead).
- Windows allows only one account per server at a time; Nexdir shows *Other account in use* and
  does not disconnect the other connection.
- No Windows code signing yet (see above).

## Files

| File | Use |
|---|---|
| `Nexdir-0.6.0-alpha.1-Setup.exe` | Installer for the current user, no administrator rights. **Recommended.** |
| `Nexdir-0.6.0-alpha.1.msi` | Windows Installer package for all users (administrator rights) |
| `Nexdir-0.6.0-alpha.1-Portable.zip` | Portable version, no installation (includes `LICENSE.txt` and `THIRD-PARTY-NOTICES.txt`) |
| `Nexdir-0.6.0-alpha.1-Setup.exe.sig`, `Nexdir-0.6.0-alpha.1.msi.sig` | Update signatures, used by Nexdir's updater |
| `SHA256SUMS.txt` | SHA-256 checksums of the files above |

**Requirements:** Windows 10 or 11, 64-bit, with the Microsoft Edge WebView2 Runtime (part of
Windows 11; the installers add it if missing; the portable version needs it installed).

License: MIT. Full list of changes: [CHANGELOG.md](https://github.com/Kervent/nexdir/blob/main/CHANGELOG.md).
