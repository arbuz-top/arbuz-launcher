# Build the custom installer (installer/arbuz.nsi) with makensis.
# Keep this file ASCII-only.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot

$conf = Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$version = [string]$conf.version
$quad = (($version -split '\.') + @('0', '0', '0'))[0..3] -join '.'

$exe = Join-Path $root 'src-tauri\target\release\arbuz-launcher.exe'
if (-not (Test-Path $exe)) {
  throw "Launcher binary not found: $exe. Run 'pnpm tauri build --no-bundle' first."
}

$candidates = @(
  (Join-Path $env:LOCALAPPDATA 'tauri\NSIS\Bin\makensis.exe'),
  (Join-Path $env:LOCALAPPDATA 'tauri\NSIS\makensis.exe'),
  'C:\Program Files (x86)\NSIS\makensis.exe',
  'C:\Program Files\NSIS\makensis.exe'
)
$makensis = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $makensis) {
  $cmd = Get-Command makensis -ErrorAction SilentlyContinue
  if ($cmd) { $makensis = $cmd.Source }
}
if (-not $makensis) {
  throw 'makensis not found. Install NSIS (choco install nsis) or run pnpm tauri build once so Tauri downloads it.'
}

$nsisRoot = Split-Path $makensis
while ($nsisRoot -and -not (Test-Path (Join-Path $nsisRoot 'Contrib\Language files'))) {
  $parent = Split-Path $nsisRoot
  if ($parent -eq $nsisRoot) { break }
  $nsisRoot = $parent
}
$langDst = Join-Path $nsisRoot 'Contrib\Language files'
$langSrc = Join-Path $root 'installer\lang'
if ((Test-Path $langDst) -and (Test-Path $langSrc)) {
  Copy-Item (Join-Path $langSrc '*.nlf') $langDst -Force
  Copy-Item (Join-Path $langSrc '*.nsh') $langDst -Force
  Write-Host "custom languages copied to: $langDst"
}

$outDir = Join-Path $root 'src-tauri\target\release\bundle\arbuz'
New-Item -ItemType Directory -Force $outDir | Out-Null

Write-Host "makensis: $makensis"
Write-Host "version:  $version ($quad)"

& $makensis "/DROOT=$root" "/DAPP_VERSION=$version" "/DAPP_VER4=$quad" (Join-Path $root 'installer\arbuz.nsi')
if ($LASTEXITCODE -ne 0) {
  throw "makensis exited with code $LASTEXITCODE"
}

Get-ChildItem $outDir | Where-Object { $_.Extension -eq '.exe' } | ForEach-Object {
  Write-Host ("done: {0}  {1:N0} bytes" -f $_.FullName, $_.Length)
}
