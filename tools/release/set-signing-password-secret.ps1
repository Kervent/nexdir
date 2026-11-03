<#
.SYNOPSIS
  Sets the GitHub secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD (and optionally the key), after
  checking the password locally.

.DESCRIPTION
  Reads the key password with a hidden prompt, signs a throwaway file with the local key and
  verifies it against the public key in tauri.conf.json. Only if that works is the password sent
  to GitHub — through gh's standard input, byte for byte (UTF-8, no added newline), never on a
  command line or on screen. Afterwards run "Verify update signing key" in GitHub Actions.

  -Environment release  stores the secrets in that GitHub environment instead of the repository
  -IncludeKey           also stores TAURI_SIGNING_PRIVATE_KEY (read from the key file)
#>
[CmdletBinding()]
param(
  [string]$KeyFile = (Join-Path $env:USERPROFILE "NexdirKeys\nexdir-update.key"),
  [string]$Repo = "Kervent/nexdir",
  [string]$Environment,
  [switch]$IncludeKey
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
if (-not (Test-Path -LiteralPath $KeyFile)) { throw "Key file not found: $KeyFile" }

$secure = Read-Host "Password of the Nexdir update key" -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("nexdir-pwcheck-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory $tmp | Out-Null
try {
  $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
  Write-Host "Password length: $($plain.Length) characters"

  # 1. Local check: sign a test file and verify it against the public key in the repository.
  $file = Join-Path $tmp "check.txt"
  [IO.File]::WriteAllText($file, "Nexdir password check")
  $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $KeyFile
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $plain
  Push-Location $root
  try {
    & npx tauri signer sign --app-version 0.0.0-check $file *> (Join-Path $tmp "sign.log")
    $signed = $LASTEXITCODE -eq 0
  } finally {
    Pop-Location
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PATH, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
  }
  if (-not $signed) { throw "This password does not open the local key (nothing was sent to GitHub)." }
  & python (Join-Path $PSScriptRoot "update_feed.py") --verify $file "$file.sig" --version 0.0.0-check
  if ($LASTEXITCODE -ne 0) { throw "The local key does not match the public key in tauri.conf.json (nothing was sent to GitHub)." }
  Write-Host "Password checked locally: it opens the key." -ForegroundColor Green

  # 2. Send exactly these bytes to GitHub through standard input.
  $target = if ($Environment) { "--repo $Repo --env $Environment" } else { "--repo $Repo" }
  function Set-Secret([string]$name, [byte[]]$bytes) {
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = (Get-Command gh).Source
    $psi.Arguments = "secret set $name $target"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $p = [Diagnostics.Process]::Start($psi)
    $p.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    $p.StandardInput.Close()
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) { throw "gh secret set $name failed (exit $($p.ExitCode))." }
  }
  if ($IncludeKey) { Set-Secret "TAURI_SIGNING_PRIVATE_KEY" ([IO.File]::ReadAllBytes($KeyFile)) }
  Set-Secret "TAURI_SIGNING_PRIVATE_KEY_PASSWORD" ([Text.Encoding]::UTF8.GetBytes($plain))
} finally {
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
  $plain = $null
  Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
