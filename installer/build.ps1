# Собирает release-лаунчер и однoфайловый установщик installer/dist/Quarry-Setup-<версия>.exe.
#
# Установщик встраивает quarry.exe в себя (include_bytes!), поэтому лаунчер
# обязан быть собран до сборки установщика. Коды возврата cargo проверяются:
# иначе можно получить «успех» со старым exe на диске.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $PSScriptRoot 'src-tauri\Cargo.toml'
$verLine = Select-String -Path $manifest -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $verLine) { throw "Не найдена версия в $manifest" }
$version = $verLine.Matches[0].Groups[1].Value

$launcherExe = Join-Path $root 'src-tauri\target\release\quarry.exe'
$installerExe = Join-Path $PSScriptRoot 'src-tauri\target\release\quarry-installer.exe'

Push-Location $root
try {
    # rustc и линкер пишут временные файлы в %TEMP%. Если системный диск забит,
    # сборка падает без внятной ошибки — уводим времянку на диск проекта.
    $tmpRoot = [IO.Path]::GetPathRoot($env:TEMP)
    $tmpName = $tmpRoot.TrimEnd('\').TrimEnd(':')
    $tmpDrive = Get-PSDrive -Name $tmpName -ErrorAction SilentlyContinue
    if ($tmpDrive -and ($tmpDrive.Free / 1GB) -lt 2) {
        $localTmp = Join-Path $PSScriptRoot '.tmp-build'
        New-Item -ItemType Directory -Force -Path $localTmp | Out-Null
        $env:TEMP = $localTmp
        $env:TMP = $localTmp
        $free = [math]::Round($tmpDrive.Free / 1GB, 1)
        Write-Host "==> На $tmpRoot свободно $free ГБ — времянка сборки перенесена в $localTmp" -ForegroundColor Yellow
    }

    Write-Host '==> Сборка фронтенда и лаунчера (release)' -ForegroundColor Cyan
    pnpm tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "pnpm tauri build завершился с кодом $LASTEXITCODE" }
    if (-not (Test-Path $launcherExe)) { throw "Не собран лаунчер: $launcherExe" }

    Write-Host '==> Сборка установщика (release)' -ForegroundColor Cyan
    cargo build --release --manifest-path installer/src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw "cargo build завершился с кодом $LASTEXITCODE" }
    if (-not (Test-Path $installerExe)) { throw "Не собран установщик: $installerExe" }

    $dist = Join-Path $PSScriptRoot 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $dst = Join-Path $dist "Quarry-Setup-$version.exe"
    Copy-Item $installerExe $dst -Force

    $mb = [math]::Round((Get-Item $dst).Length / 1MB, 1)
    Write-Host "==> Готово: $dst ($mb МБ)" -ForegroundColor Green
}
finally {
    Pop-Location
}
