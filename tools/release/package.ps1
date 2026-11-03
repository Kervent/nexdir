<#
.SYNOPSIS
  Collects the release files after `npx tauri build` and writes SHA256SUMS.txt.

.DESCRIPTION
  Used by .github/workflows/release.yml and for local release builds, so both produce the
  same file set:
    Nexdir-<version>-Setup.exe     NSIS installer (per user, no admin rights)
    Nexdir-<version>.msi           MSI package
    Nexdir-<version>-Portable.zip  Nexdir.exe + LICENSE.txt + THIRD-PARTY-NOTICES.txt + README.txt
    Nexdir-<version>-Setup.exe.sig Tauri update signatures of both installers (only when the
    Nexdir-<version>.msi.sig       build ran with the update signing key, see docs/updates/signing.md)
    SHA256SUMS.txt                 SHA-256 of all files above

  Three different things protect the downloads and must not be confused: SHA-256 checksums
  (detect damaged downloads), Tauri update signatures (Nexdir's updater only installs files
  signed with the Nexdir update key) and Windows Authenticode code signing (not used yet).

  Only installers whose file name contains the version from tauri.conf.json are used, so a
  stale bundle from an earlier build can never be shipped. If code signing is added later,
  it must happen before this script runs: the checksums are computed last.

.EXAMPLE
  npm run app:build
  powershell -ExecutionPolicy Bypass -File tools/release/package.ps1
#>
[CmdletBinding()]
param(
  [string]$OutDir = "release-out",
  # Replace an existing output folder (only that folder is removed).
  [switch]$Clean,
  # Release builds: fail unless both installers have a valid update signature for this version.
  [switch]$RequireSignatures,
  # Public key to check the signatures with; default: the one in tauri.conf.json. Only test
  # builds made with a test key (docs/updates/testing.md) pass one.
  [string]$PublicKey
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$tauriDir = Join-Path $root "src-tauri"
$bundle = Join-Path $tauriDir "target\release\bundle"
$version = (Get-Content (Join-Path $tauriDir "tauri.conf.json") -Raw | ConvertFrom-Json).version
if (-not $version) { throw "No version in src-tauri/tauri.conf.json" }

if (-not [IO.Path]::IsPathRooted($OutDir)) { $OutDir = Join-Path $root $OutDir }
if (Test-Path $OutDir) {
  if (-not $Clean) { throw "$OutDir already exists. Use -Clean to replace it." }
  Remove-Item -LiteralPath $OutDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutDir | Out-Null

function Find-One([string]$folder, [string]$filter) {
  $found = @(Get-ChildItem -Path (Join-Path $bundle $folder) -Filter $filter -File -ErrorAction SilentlyContinue)
  if ($found.Count -ne 1) { throw "Expected exactly one $filter in bundle\$folder, found $($found.Count). Run 'npm run app:build' first." }
  return $found[0]
}

$setup = Find-One "nsis" "Nexdir_${version}_x64-setup.exe"
# The MSI file is named after the app version, also for pre-releases whose MSI-internal
# version is numeric (bundle.windows.wix.version, e.g. 0.5.0.1).
$msi = Find-One "msi" "Nexdir_${version}_x64_*.msi"
$exe = Join-Path $tauriDir "target\release\nexdir.exe"
if (-not (Test-Path $exe)) { throw "Missing $exe" }
# The bundles embed the exe; an exe newer than the installers means the bundles are stale.
if ((Get-Item $exe).LastWriteTimeUtc -gt $setup.LastWriteTimeUtc.AddMinutes(1)) {
  throw "nexdir.exe is newer than the installer. Run 'npm run app:build' again."
}

$files = @(
  @{ Source = $setup.FullName; Name = "Nexdir-$version-Setup.exe" },
  @{ Source = $msi.FullName; Name = "Nexdir-$version.msi" }
)
foreach ($f in $files) { Copy-Item -LiteralPath $f.Source -Destination (Join-Path $OutDir $f.Name) }

# Update signatures (.sig next to each installer, written by `tauri build` when
# bundle.createUpdaterArtifacts is on and TAURI_SIGNING_PRIVATE_KEY is set).
foreach ($f in $files) {
  $sig = "$($f.Source).sig"
  if (-not (Test-Path -LiteralPath $sig)) {
    if ($RequireSignatures) { throw "Missing update signature $sig. Unsigned update files are not released." }
    Write-Warning "$($f.Name) has no update signature (local build without the signing key)."
    continue
  }
  if ((Get-Item -LiteralPath $sig).LastWriteTimeUtc -lt (Get-Item -LiteralPath $f.Source).LastWriteTimeUtc.AddMinutes(-1)) {
    throw "$sig is older than its installer (stale signature)."
  }
  # Checked against the public key in tauri.conf.json, and bound to exactly this version.
  $keyArgs = if ($PublicKey) { @("--pubkey", $PublicKey) } else { @() }
  & python (Join-Path $PSScriptRoot "update_feed.py") --verify $f.Source $sig --version $version @keyArgs
  if ($LASTEXITCODE -ne 0) { throw "The update signature of $($f.Name) is invalid." }
  Copy-Item -LiteralPath $sig -Destination (Join-Path $OutDir "$($f.Name).sig")
}

# Portable ZIP
$staging = Join-Path ([IO.Path]::GetTempPath()) ("nexdir-portable-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $staging | Out-Null
try {
  Copy-Item -LiteralPath $exe -Destination (Join-Path $staging "Nexdir.exe")
  Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination (Join-Path $staging "LICENSE.txt")
  # Licenses of the bundled third-party components must ship with the binary.
  Copy-Item -LiteralPath (Join-Path $root "THIRD-PARTY-NOTICES.txt") -Destination (Join-Path $staging "THIRD-PARTY-NOTICES.txt")
  Copy-Item -LiteralPath (Join-Path $root "docs\release\PORTABLE.txt") -Destination (Join-Path $staging "README.txt")
  Compress-Archive -Path (Join-Path $staging "*") -DestinationPath (Join-Path $OutDir "Nexdir-$version-Portable.zip")
} finally {
  Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
}

# Checksums: "<lowercase sha256>  <name>", LF line endings, like sha256sum.
$lines = Get-ChildItem -Path $OutDir -File | Where-Object Name -ne "SHA256SUMS.txt" | Sort-Object Name | ForEach-Object {
  "{0}  {1}" -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name
}
[IO.File]::WriteAllText((Join-Path $OutDir "SHA256SUMS.txt"), (($lines -join "`n") + "`n"), [Text.Encoding]::ASCII)

Write-Host "Nexdir $version release files in ${OutDir}:"
Get-ChildItem -Path $OutDir -File | ForEach-Object { "  {0,-32} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB) }
Write-Host ""
Get-Content (Join-Path $OutDir "SHA256SUMS.txt")
