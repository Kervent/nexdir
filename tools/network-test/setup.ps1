# Creates the temporary Samba test server for Nexdir's network tests (see README.md).
# - a separate WSL distro "nexdir-samba-test" (Debian) in -Location; existing distros are not used
# - random passwords for the test accounts in .\secrets\creds.json (git-ignored, never printed)
# - Samba bound only to the distro's virtual NAT interface (SMB3, signing required, no guest)
param(
  [Parameter(Mandatory)][string]$Location
)
$ErrorActionPreference = "Stop"
$Distro = "nexdir-samba-test"
if ((wsl.exe -l -q) -replace "`0", "" | Where-Object { $_.Trim() -eq $Distro }) { throw "$Distro already exists. Run cleanup.ps1 first or reuse it." }
if ((Get-Content "$env:USERPROFILE\.wslconfig" -ErrorAction SilentlyContinue) -match 'networkingMode\s*=\s*mirrored') {
  throw "WSL uses mirrored networking: the test server would be reachable on the LAN and collide with Windows' port 445. Not supported."
}
New-Item -ItemType Directory -Force $Location | Out-Null
wsl.exe --install Debian --name $Distro --location $Location --no-launch
$secrets = Join-Path $PSScriptRoot "secrets"
New-Item -ItemType Directory -Force $secrets | Out-Null
$alphabet = [char[]]"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789"
$creds = [ordered]@{}
foreach ($u in "nxtest", "nxprivate", "nxother") {
  $bytes = New-Object byte[] 24
  [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
  $creds[$u] = -join ($bytes | ForEach-Object { $alphabet[$_ % $alphabet.Length] })
}
$creds | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $secrets "creds.json")
$wslHere = (wsl.exe -d $Distro -u root -- wslpath -a ($PSScriptRoot -replace '\\', '/')) -replace "`0", ""
wsl.exe -d $Distro -u root -- bash -c "export DEBIAN_FRONTEND=noninteractive; apt-get update -qq && apt-get install -y -qq samba smbclient iproute2 >/dev/null"
wsl.exe -d $Distro -u root -- bash "$wslHere/provision.sh" "$wslHere/secrets"
$ip = ((wsl.exe -d $Distro -u root -- bash -c "ip -4 -o addr show eth0 | awk '{print `$4}' | cut -d/ -f1") -replace "`0", "").Trim()
"Samba test server: \\$ip  (shares PublicTest, PrivateTest, ReadOnlyTest, LargeFiles)"
"Keep it running while testing:  Start-Process wsl.exe -ArgumentList '-d','$Distro','-u','root','--','sleep','infinity' -WindowStyle Hidden"
