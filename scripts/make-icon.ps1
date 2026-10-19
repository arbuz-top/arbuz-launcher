# Генерирует иконки Tauri из assets/launcher/icon.png.
# Маленькие иконки (например, 16x16) увеличиваются методом ближайшего соседа,
# поэтому пиксель-арт остаётся чётким.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$candidates = @(
    (Join-Path $root 'assets\launcher\icon.png'),
    (Join-Path $root 'assets\icon.png')
)
$srcPath = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
$outPng = Join-Path $root 'src-tauri\icons\icon.png'
$outIco = Join-Path $root 'src-tauri\icons\icon.ico'

if (-not $srcPath) {
    Write-Host "make-icon: assets\launcher\icon.png не найден, оставляю текущие иконки."
    exit 0
}

$src = [System.Drawing.Image]::FromFile($srcPath)

function Resize-Icon([System.Drawing.Image]$image, [int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::None
    $g.DrawImage($image, 0, 0, $size, $size)
    $g.Dispose()
    return $bmp
}

try {
    $big = Resize-Icon $src 1024
    $big.Save($outPng, [System.Drawing.Imaging.ImageFormat]::Png)
    $big.Dispose()

    $sizes = @(16, 24, 32, 48, 64, 128, 256)
    $pngs = @()
    foreach ($s in $sizes) {
        $b = Resize-Icon $src $s
        $ms = New-Object System.IO.MemoryStream
        $b.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $pngs += , @($s, $ms.ToArray())
        $b.Dispose()
        $ms.Dispose()
    }

    $out = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($out)
    $bw.Write([UInt16]0)
    $bw.Write([UInt16]1)
    $bw.Write([UInt16]$pngs.Count)
    $offset = 6 + 16 * $pngs.Count
    foreach ($p in $pngs) {
        $s = [int]$p[0]
        $data = [byte[]]$p[1]
        $dim = if ($s -ge 256) { 0 } else { $s }
        $bw.Write([Byte]$dim)
        $bw.Write([Byte]$dim)
        $bw.Write([Byte]0)
        $bw.Write([Byte]0)
        $bw.Write([UInt16]1)
        $bw.Write([UInt16]32)
        $bw.Write([UInt32]$data.Length)
        $bw.Write([UInt32]$offset)
        $offset += $data.Length
    }
    foreach ($p in $pngs) {
        $bw.Write([byte[]]$p[1])
    }
    $bw.Flush()
    [System.IO.File]::WriteAllBytes($outIco, $out.ToArray())
    $bw.Dispose()
    $out.Dispose()

    # Копия для заголовка окна (titlebar).
    $pub = Join-Path $root 'public\launcher.png'
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $pub) | Out-Null
    Copy-Item -LiteralPath $srcPath -Destination $pub -Force

    Write-Host "make-icon: готово -> src-tauri/icons/icon.png, icon.ico, public/launcher.png"
}
finally {
    $src.Dispose()
}
