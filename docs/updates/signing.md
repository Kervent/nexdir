# Signing updates

## Three different protections

| Mechanism | Protects against | Checked by | Status |
|---|---|---|---|
| **Tauri update signature** (minisign / Ed25519, `.sig` files) | Installing anything not made by the Nexdir maintainers through the in-app updater, including tampered downloads and a tampered feed pairing a new version number with an old file | Nexdir's updater, before installing; the update-feed workflow before listing a release | From 0.5.0 |
| **Windows Authenticode code signing** (certificate from a CA) | SmartScreen warnings; shows the publisher in Windows | Windows | Not yet ([code-signing.md](../release/code-signing.md)) |
| **SHA-256 checksums** (`SHA256SUMS.txt`) | Damaged downloads; lets people compare files by hand | People (`Get-FileHash`) and the feed workflow | Since 0.1 |

They are independent. An update signature does not make SmartScreen trust the installer, and a
checksum from the same page as the file proves nothing about who made it.

## The update key

- **Public key**: `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. It is built into
  every Nexdir binary and is public on purpose.
- **Private key** (password-protected minisign secret key): signs the installers during the
  release workflow. It must **never** be in the repository, in a build log, a screenshot, a
  configuration file, an issue, or inside release files.

The updater runs with `requireSignedVersion`: the signed *trusted comment* of each `.sig` must
contain `version:<exact version>`. The Tauri CLI writes it automatically. A signature for 0.5.0
therefore cannot be reused to pass 0.5.0 off as 0.6.0.

The production key was created on 2026-11-03 (minisign key ID `B64E9EF8C4E690D2`) with
`tools/release/new-update-key.ps1`; its public half is in `tauri.conf.json`, the private key and
password are GitHub Actions secrets and kept offline by the owner.

### Creating the production key (one time, maintainer only)

Do this on a trusted PC, not in CI. `tools/release/new-update-key.ps1` does all of the steps below:
it refuses folders inside the repository or OneDrive, restricts the folder to your account, never
overwrites an existing key, prints only the public key, and optionally sets the secrets (the key
is read from the file, the password typed into gh's hidden prompt — paste **only the password**
there). By hand:

```powershell
# 1. Generate (asks for a password; use a long random one from a password manager).
npx tauri signer generate -w "$env:USERPROFILE\NexdirKeys\nexdir-update.key"
# 2. Put the PUBLIC key (nexdir-update.key.pub, one line of base64) into
#    src-tauri/tauri.conf.json -> plugins.updater.pubkey and commit it.
# 3. Store the PRIVATE key and password as GitHub Actions secrets (read from the file, never typed
#    into a command line or chat):
Get-Content "$env:USERPROFILE\NexdirKeys\nexdir-update.key" -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo Kervent/nexdir
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo Kervent/nexdir   # prompts for the value
```

Restrict the secrets to the release workflow by using a GitHub *environment* with required
reviewers if more maintainers join.

### Backup

Losing the private key means installed copies can no longer be updated in-app (users would have
to install a new version by hand once). Keep **two offline copies** of the key file and its
password, separately: for example an encrypted USB stick in a safe place and a password manager
entry holding both. Never store the key unencrypted in cloud sync folders.

### Rotation

Rotate when the key or password may have leaked, or when a maintainer with access leaves.

1. Generate a new key pair (as above).
2. Release a version that contains the **new** public key but is signed with the **old** key —
   installed copies can only verify the old key and update to it.
3. Replace the secrets with the new private key; every later release is signed with it.
4. If the old key was compromised: publish a security advisory, release step 2 as soon as
   possible, and consider withdrawing (unpublishing) releases signed after the compromise — the
   feed workflow then drops them automatically.

Copies older than the rotation release must be updated manually once if step 2 is skipped.

### Setting or checking the password secret

Pasting into gh's interactive prompt lost a character once (2026-11-03). Use
`tools/release/set-signing-password-secret.ps1` instead: it reads the password with a hidden
PowerShell prompt, proves locally that it opens the key (signs and verifies a test file), and only
then passes the exact bytes to `gh secret set` through standard input. Afterwards run
*Actions → Verify update signing key*, which signs a throwaway file on GitHub with both secrets
and verifies it against the public key (nothing is built or published).

## Signed builds on your own PC

`powershell -ExecutionPolicy Bypass -File tools/release/build-signed.ps1` asks for the key password
(hidden), builds with update signatures, verifies them and writes the release files to
`release-out\<version>\`. The key and password exist only in that process's environment.

## What the release workflow enforces

- The build stops when `pubkey` is empty or the `TAURI_SIGNING_PRIVATE_KEY` secret is missing.
- `tauri.release.conf.json` turns on `createUpdaterArtifacts`, so the CLI writes a `.sig` next to
  each installer. The key is passed only to that step as an environment variable.
- `tools/release/package.ps1 -RequireSignatures` refuses missing or stale `.sig` files and
  verifies each against the public key and the version (`tools/release/update_feed.py --verify`).
- The release is created as a **draft**. Nothing reaches users until a maintainer publishes it;
  only then does the update-feed workflow list it — after checking the signatures once more.
