# Release smoke test

Run on a clean Windows test machine or VM, with **test folders only** (create a few folders and
files first; never test with personal data). Tick each line before publishing a draft release.

## Files

- [ ] `SHA256SUMS.txt` matches all three downloads (`Get-FileHash <file> -Algorithm SHA256`).
- [ ] Installer, MSI and `Nexdir.exe` show the release version (*Properties → Details*).
- [ ] If signed: `Get-AuthenticodeSignature <file>` reports `Valid` for all three.

## Install and start

- [ ] Setup installs without administrator prompt to `%LOCALAPPDATA%\Nexdir`; Start menu and
      desktop shortcuts exist; *Settings → Apps* lists Nexdir with the release version.
- [ ] Uninstall removes the program and shortcuts but keeps settings in
      `%APPDATA%\org.nexdir.desktop` (unless "delete app data" is ticked).
- [ ] Nexdir starts; on a fresh profile the *Welcome to Nexdir* dialog appears once (Skip for Now keeps it away).
- [ ] *Help → About Nexdir* shows the release version; *Help → Report a Bug* review shows version and Windows details and opens the prefilled GitHub form.
- [ ] Portable ZIP: `Nexdir.exe` starts from an extracted folder without installation.
- [ ] Windows 10 test (if available): starts, WebView2 present or installed by the setup.

## File management (in a test folder)

- [ ] Browse folders, back/forward/up, address bar with `%USERPROFILE%`.
- [ ] New folder, rename (also case-only, e.g. `a.txt` → `A.txt`), undo rename.
- [ ] Copy and move with a conflict: *Keep both*, *Skip*, *Replace* behave as described.
- [ ] Cancel a large copy: no partial file remains.
- [ ] Delete → item is in the Recycle Bin and can be restored.
- [ ] Copy in Nexdir, paste in Explorer (and the other way round).
- [ ] Tabs, split view, drag and drop between panes.
- [ ] Opening a `.bat` or `.exe` asks for confirmation first.

## Tools (in a test folder)

- [ ] Search with a sentence ("text files larger than 1 KB") shows chips and results.
- [ ] Storage analysis completes and can be cancelled.
- [ ] Duplicate finder finds a copied file; refuses to remove every copy of a group.
- [ ] Organize rule: preview lists files, nothing changes until confirmed.

## Appearance

- [ ] Light, dark and system theme; text readable in both.
- [ ] Window at minimum size: no overlapping controls; 150 % display scaling looks sharp.

## After publishing

- [ ] Discord announcement appeared once in #releases (if webhooks are configured).
- [ ] Download links in the announcement work.
