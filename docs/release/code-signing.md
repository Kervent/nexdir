# Code signing

Nexdir releases are currently **unsigned**. Windows SmartScreen may therefore show
"Windows protected your PC" until a release builds reputation or is signed. This is expected;
Nexdir never tries to bypass or weaken Windows security mechanisms. Users should verify
downloads with `SHA256SUMS.txt`:

```powershell
Get-FileHash .\Nexdir-0.4.0-Setup.exe -Algorithm SHA256
# compare with the line for this file in SHA256SUMS.txt
```

Then choose *More info → Run anyway* only if the checksum matches.

## What is already prepared

- **One place for signing in the build.** Tauri runs `bundle.windows.signCommand` for every
  binary it produces — `nexdir.exe` *before* it is packed into the installers, the NSIS
  uninstaller and the installers themselves. Signing therefore needs no change to the build
  steps, only configuration.
- **Checksums come last.** [`tools/release/package.ps1`](../../tools/release/package.ps1)
  computes `SHA256SUMS.txt` after the build, so the checksums always describe the signed files.
- **Correct metadata.** Product name, version, publisher ("Nexdir contributors") and the
  application manifest (`asInvoker`, Windows 10/11, long-path aware —
  [`src-tauri/windows-app-manifest.xml`](../../src-tauri/windows-app-manifest.xml)) are
  embedded, which signing services check.
- **Separation of duties.** Releases are built by the Release workflow and published by hand
  (draft first). A signing secret would live in its own GitHub environment with a required
  reviewer, like the Discord webhooks.

## Options

| Option | Cost | How it plugs in | Notes |
|---|---|---|---|
| [SignPath Foundation](https://signpath.org) | Free for qualifying OSS | CI uploads the unsigned build to SignPath; their action returns signed files | Needs a public repository with an OSI license (MIT ✓), MFA for maintainers, builds from CI only (no local builds). Certificate is issued to SignPath Foundation, publisher shown as "SignPath Foundation". |
| Microsoft's managed signing service in Azure ("Trusted Signing") | Monthly subscription | `signCommand` calls the signing client with the file path | Identity validation required; keys never leave Azure. Check the current name and availability before starting. |
| OV/EV certificate from a CA | Yearly fee | `signCommand` runs `signtool` against an HSM or cloud key vault | Private keys must be on a hardware token/HSM since 2023. An EV certificate no longer skips SmartScreen reputation building, so OV is usually enough. |

## Steps when a signing option is chosen

1. Create a GitHub environment `release-signing` with the project owner as required reviewer;
   store the credentials there (never in the repository).
2. Add `signCommand` to `src-tauri/tauri.conf.json` under `bundle.windows`. Its shape
   (placeholder values; the real command and arguments come from the provider's docs):
   ```json
   "signCommand": "<signing tool> sign <provider options> --timestamp <provider timestamp URL> %1"
   ```
   `%1` is replaced by the path of the file to sign. Always timestamp the signature so it
   stays valid after the certificate expires.
   For SignPath, instead add their GitHub action after `npx tauri build` and sign the exe,
   the installer and the MSI there, before `package.ps1` runs.
3. Use `environment: release-signing` on the release job.
4. Verify locally after downloading the draft:
   ```powershell
   Get-AuthenticodeSignature .\Nexdir-<version>-Setup.exe | Format-List Status, SignerCertificate
   ```
   `Status` must be `Valid`. Also check the MSI and the `Nexdir.exe` inside the portable ZIP.
5. Update this page and the README download section.

## Never

- Publish with a self-signed certificate (it is not trusted and only confuses users).
- Commit keys, certificates or passwords, or print them in workflow logs.
- Ask users to turn off SmartScreen or Defender.
