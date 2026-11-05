# Network test environment (temporary Samba server)

Reproducible SMB test server for Nexdir's network feature, without NAS hardware. It runs Samba
in a **separate WSL distro** (`nexdir-samba-test`, Debian) and is reached by Windows through its
own SMB client over WSL's virtual NAT network. This is a *virtual network integration test*, not a
test with NAS hardware.

## Isolation

- WSL's default NAT networking: the distro has its own address on a host-internal virtual switch
  (e.g. `172.29.37.90`). Other devices in the LAN cannot reach it; no port forwarding is set up.
  `setup.ps1` refuses WSL's *mirrored* networking (that would expose the server on the LAN and
  collide with Windows' own port 445).
- Samba listens only on the distro's `eth0` (`bind interfaces only`), port 445 only, no NetBIOS,
  SMB3 minimum, signing required, no guest access (`map to guest = Never`, `restrict anonymous = 2`).
- Windows' own SMB server, shares, firewall and security settings are not changed. Your other
  WSL distros are not touched.

## Shares and accounts

| Share | Purpose | Allowed accounts |
|---|---|---|
| `PublicTest` | read/write operations | `nxtest`, `nxprivate` |
| `PrivateTest` | password-protected, single user | `nxprivate` |
| `ReadOnlyTest` | read-only (writes must fail) | `nxtest`, `nxprivate` |
| `LargeFiles` | performance, many/large files | `nxtest` |

`nxother` exists but has access to no share. Passwords are random, stored only in
`secrets/creds.json` (git-ignored) and never printed or passed on a command line.

## Use

```powershell
.\setup.ps1 -Location D:\WSL\nexdir-samba-test          # once
Start-Process wsl.exe -ArgumentList '-d','nexdir-samba-test','-u','root','--','sleep','infinity' -WindowStyle Hidden
.\credhelper.ps1 -Action set-cred -User nxtest            # session-only Windows credential
$env:NEXDIR_SAMBA_SERVER = "172.29.37.90"                # address printed by setup.ps1
cargo test --manifest-path ..\..\src-tauri\Cargo.toml --lib samba_user -- --ignored --test-threads=1 --nocapture
.\cleanup.ps1                                            # removes connections, credential, distro, secrets
```

Test phases (`src-tauri/src/network/samba_tests.rs`): `samba_nocred_*` (no stored credentials),
`samba_user_*` (credentials of `nxtest`), `samba_outage_*` (stops and starts smbd in the distro),
`samba_slow_*` (with `NEXDIR_SAMBA_NETEM`, e.g. `rate 20mbit`, applied with `tc netem` to the
distro's interface in both directions), `samba_existing_*` (an existing connection as `nxprivate`,
made with `credhelper.ps1 -Action connect-as`), `samba_drive_*` (with `NEXDIR_SAMBA_DRIVE` mapped
to `PublicTest`).

Windows' own sign-in dialog is not automated by these tools: entering credentials there is done by
a person (see docs/testing/network-storage-tests.md).
