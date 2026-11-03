<#
.SYNOPSIS
  Creates Nexdir's update signing key (minisign / Ed25519) on this PC. Maintainer only.

.DESCRIPTION
  Run it yourself in a terminal: the Tauri CLI asks for the key password with hidden input.
  The private key is written outside the repository (default %USERPROFILE%\NexdirKeys), readable
  only by your Windows account. Only the PUBLIC key is printed. An existing key is never
  overwritten. Optionally stores the private key and its password as GitHub Actions secrets
  (read from the file / typed into gh's hidden prompt, never placed on a command line).

  See docs/updates/signing.md (backup, rotation).
#>
[CmdletBinding()]
param(
  [string]$KeyDir = (Join-Path $env:USERPROFILE "NexdirKeys"),
  [string]$Repo = "Kervent/nexdir"
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$key = Join-Path $KeyDir "nexdir-update.key"
$pub = "$key.pub"

if ([IO.Path]::GetFullPath($KeyDir).StartsWith([IO.Path]::GetFullPath($root), [StringComparison]::OrdinalIgnoreCase)) {
  throw "The key folder must be outside the repository."
}
foreach ($sync in @($env:OneDrive, $env:OneDriveConsumer, $env:OneDriveCommercial)) {
  if ($sync -and [IO.Path]::GetFullPath($KeyDir).StartsWith([IO.Path]::GetFullPath($sync), [StringComparison]::OrdinalIgnoreCase)) {
    throw "The key folder is inside a OneDrive folder ($sync). Choose a folder that is not synced to the cloud (-KeyDir)."
  }
}
if (Test-Path -LiteralPath $key) { throw "A key already exists at $key. It is never overwritten; see docs/updates/signing.md for rotation." }

New-Item -ItemType Directory -Force -Path $KeyDir | Out-Null
# Only the current user (and SYSTEM) may access the folder.
& icacls $KeyDir /inheritance:r /grant:r "${env:USERNAME}:(OI)(CI)F" "SYSTEM:(OI)(CI)F" | Out-Null

Write-Host ""
Write-Host "Creating the Nexdir update key in $KeyDir"
Write-Host "Choose a long, random password (for example from a password manager) and store it there." -ForegroundColor Yellow
Write-Host ""
Push-Location $root
try {
  & npx tauri signer generate -w $key
  if ($LASTEXITCODE -ne 0) { throw "Key generation failed (exit $LASTEXITCODE)." }
} finally {
  Pop-Location
}
if (-not (Test-Path -LiteralPath $key) -or -not (Test-Path -LiteralPath $pub)) { throw "The key files were not created." }

Write-Host ""
Write-Host "Done. Files:" -ForegroundColor Green
Write-Host "  private key (secret!): $key"
Write-Host "  public key:            $pub"
Write-Host ""
Write-Host "Public key (goes into src-tauri/tauri.conf.json, plugins.updater.pubkey):"
Write-Host (Get-Content -LiteralPath $pub -Raw).Trim()
Write-Host ""
Write-Host "BACKUP NOW: copy nexdir-update.key to two offline places (e.g. an encrypted USB stick) and keep" -ForegroundColor Yellow
Write-Host "the password in your password manager. Without them, installed copies cannot be updated in-app." -ForegroundColor Yellow
Write-Host ""

$answer = Read-Host "Store the private key and its password as GitHub Actions secrets of $Repo now? (y/N)"
if ($answer -match '^(y|yes|j|ja)$') {
  Get-Content -LiteralPath $key -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $Repo
  if ($LASTEXITCODE -ne 0) { throw "Setting TAURI_SIGNING_PRIVATE_KEY failed." }
  Write-Host "Now enter the key password once more for the secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD:"
  gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $Repo
  if ($LASTEXITCODE -ne 0) { throw "Setting TAURI_SIGNING_PRIVATE_KEY_PASSWORD failed." }
  gh secret list --repo $Repo
} else {
  Write-Host "Secrets not set. Later: see docs/updates/signing.md."
}
