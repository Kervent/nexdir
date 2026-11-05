# Removes everything setup.ps1 and the tests created: test connections and the drive letter the
# tests mapped (only to the test server), the session credential, the WSL distro and the secrets.
param(
  [string]$Server = "172.29.37.90",
  [string]$DriveLetter = "X"
)
$Distro = "nexdir-samba-test"
$mapped = (net use) | Select-String -SimpleMatch "\\$Server\"
foreach ($line in $mapped) {
  $parts = ($line.ToString() -split '\s+') | Where-Object { $_ }
  $target = $parts | Where-Object { $_ -like "\\$Server\*" } | Select-Object -First 1
  $local = $parts | Where-Object { $_ -match '^[A-Z]:$' } | Select-Object -First 1
  if ($local -and $local -eq "${DriveLetter}:") { net use $local /delete /y | Out-Null; "removed $local -> $target" }
  elseif ($target -and -not $local) { net use $target /delete /y | Out-Null; "removed $target" }
}
& (Join-Path $PSScriptRoot "credhelper.ps1") -Action remove-cred -Server $Server
wsl.exe --terminate $Distro 2>$null
wsl.exe --unregister $Distro
$secrets = Join-Path $PSScriptRoot "secrets"
if (Test-Path (Join-Path $secrets "creds.json")) { Remove-Item (Join-Path $secrets "creds.json"); "removed secrets\creds.json" }
