<#
.SYNOPSIS
  Local release build signed with the Nexdir update key. Maintainer only; run it yourself.

.DESCRIPTION
  Asks for the key password (hidden input), passes key and password to `tauri build` through
  environment variables of this process only, builds with tauri.release.conf.json (update
  signatures on), then collects the release files with `package.ps1 -RequireSignatures`, which
  verifies both signatures against the public key in tauri.conf.json and the version.
  Output: release-out\<version>\ (ignored by Git). Nothing is uploaded.
#>
[CmdletBinding()]
param([string]$KeyFile = (Join-Path $env:USERPROFILE "NexdirKeys\nexdir-update.key"))
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$version = (Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version
$pubkey = (Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).plugins.updater.pubkey
if (-not $pubkey) { throw "tauri.conf.json has no updater public key." }
if (-not (Test-Path -LiteralPath $KeyFile)) { throw "Key file not found: $KeyFile" }
if ((Get-Content -LiteralPath "$KeyFile.pub" -Raw).Trim() -ne $pubkey.Trim()) { throw "$KeyFile.pub does not match the public key in tauri.conf.json." }
$out = Join-Path $root "release-out\$version"
if (Test-Path $out) { throw "$out exists already." }

$secure = Read-Host "Password of the Nexdir update key" -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
  $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -LiteralPath $KeyFile -Raw
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
  Push-Location $root
  try {
    & npx tauri build --config src-tauri/tauri.release.conf.json
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed (exit $LASTEXITCODE)." }
  } finally { Pop-Location }
} finally {
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}
& (Join-Path $PSScriptRoot "package.ps1") -OutDir $out -RequireSignatures
Write-Host ""
Write-Host "Signed release files for $version in $out" -ForegroundColor Green
