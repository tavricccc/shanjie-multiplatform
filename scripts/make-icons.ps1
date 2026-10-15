#Requires -Version 7
# Regenerates the mode icons: a flat, modern squircle with a soft gradient and a bold sans glyph (no inner frame).
# Chinese is a vermilion tile with 解 (the upstream seal colour); English is a deep slate tile with A.
# Needs the Noto Sans TC font; glyphs are rasterized, no font is shipped.
param([string]$Font = 'Noto Sans TC')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$out = Join-Path (Split-Path $PSScriptRoot -Parent) 'assets/mode'
if (!(New-Object System.Drawing.Text.InstalledFontCollection).Families.Name.Contains($Font)) { throw "Font not installed: $Font" }

function New-RoundedPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
    $path = [Drawing.Drawing2D.GraphicsPath]::new()
    $d = 2 * $r
    $path.AddArc($x, $y, $d, $d, 180, 90)
    $path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

function New-Icon([int]$size, [string]$glyph, [Drawing.Color]$top, [Drawing.Color]$bottom) {
    $bitmap = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear([Drawing.Color]::Transparent)
    $g.ScaleTransform($size / 64.0, $size / 64.0)
    # Soft diagonal gradient on a generously rounded tile; no frame, no serif.
    $tile = New-RoundedPath 2 2 60 60 16
    $gradient = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.PointF]::new(10, 2), [Drawing.PointF]::new(54, 62), $top, $bottom)
    $g.FillPath($gradient, $tile)
    $text = [Drawing.Drawing2D.GraphicsPath]::new([Drawing.Drawing2D.FillMode]::Winding)
    $family = [Drawing.FontFamily]::new($Font)
    $em = if ($glyph -eq 'A') { 40 } else { 37 }
    $text.AddString($glyph, $family, [int][Drawing.FontStyle]::Bold, $em, [Drawing.PointF]::new(0, 0), [Drawing.StringFormat]::GenericTypographic)
    $bounds = $text.GetBounds()
    $move = [Drawing.Drawing2D.Matrix]::new()
    $move.Translate(32 - ($bounds.X + $bounds.Width / 2), 32 - ($bounds.Y + $bounds.Height / 2))
    $text.Transform($move)
    $g.FillPath([Drawing.Brushes]::White, $text)
    $g.Dispose()
    return $bitmap
}

function Save-Ico([string]$path, $bitmaps) {
    $images = foreach ($b in $bitmaps) { $m = [IO.MemoryStream]::new(); $b.Save($m, [Drawing.Imaging.ImageFormat]::Png); ,$m.ToArray() }
    $file = [IO.MemoryStream]::new()
    $w = [IO.BinaryWriter]::new($file)
    $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$bitmaps.Count)
    $offset = 6 + 16 * $bitmaps.Count
    for ($i = 0; $i -lt $bitmaps.Count; $i++) {
        $s = $bitmaps[$i].Width
        $w.Write([byte]($s -band 255)); $w.Write([byte]($s -band 255)); $w.Write([byte]0); $w.Write([byte]0)
        $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$images[$i].Length); $w.Write([uint32]$offset)
        $offset += $images[$i].Length
    }
    foreach ($image in $images) { $w.Write($image) }
    $w.Flush()
    [IO.File]::WriteAllBytes($path, $file.ToArray())
}

$sizes = 16, 20, 24, 32, 40, 48, 64, 256
$modes = @(
    @{ Name = 'chinese'; Glyph = '解'; Top = [Drawing.Color]::FromArgb(0xF0, 0x5A, 0x44); Bottom = [Drawing.Color]::FromArgb(0xB4, 0x2B, 0x2B) },
    @{ Name = 'english'; Glyph = 'A'; Top = [Drawing.Color]::FromArgb(0x5B, 0x6B, 0x86); Bottom = [Drawing.Color]::FromArgb(0x2B, 0x35, 0x4A) })
foreach ($mode in $modes) {
    $bitmaps = @(foreach ($s in $sizes) { New-Icon $s $mode.Glyph $mode.Top $mode.Bottom })
    Save-Ico (Join-Path $out "$($mode.Name).ico") $bitmaps
    # 128 px master for the cursor-side hint, which scales it to the actual DPI.
    (New-Icon 128 $mode.Glyph $mode.Top $mode.Bottom).Save((Join-Path $out "$($mode.Name).png"), [Drawing.Imaging.ImageFormat]::Png)
    $a = '#{0:X2}{1:X2}{2:X2}' -f $mode.Top.R, $mode.Top.G, $mode.Top.B
    $b = '#{0:X2}{1:X2}{2:X2}' -f $mode.Bottom.R, $mode.Bottom.G, $mode.Bottom.B
    $svg = @"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="10" y1="2" x2="54" y2="62"><stop offset="0" stop-color="$a"/><stop offset="1" stop-color="$b"/></linearGradient></defs><rect x="2" y="2" width="60" height="60" rx="16" fill="url(#g)"/><text x="32" y="32" text-anchor="middle" dominant-baseline="central" font-family="'Noto Sans TC','Microsoft JhengHei UI',sans-serif" font-weight="700" font-size="37" fill="#fff">$($mode.Glyph)</text></svg>
"@
    Set-Content (Join-Path $out "$($mode.Name).svg") $svg.Trim() -Encoding utf8NoBOM
}

# The settings app, Installed Apps entry and tile assets use the same Chinese-mode artwork as the input method.
$ui = Join-Path (Split-Path $PSScriptRoot -Parent) 'ui/Assets'
$zh = $modes[0]
Copy-Item (Join-Path $out 'chinese.ico') (Join-Path $ui 'AppIcon.ico') -Force
function Save-Canvas([string]$name, [int]$width, [int]$height, [double]$fraction) {
    $canvas = [Drawing.Bitmap]::new($width, $height, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($canvas)
    $g.InterpolationMode = 'HighQualityBicubic'
    $side = [int]([Math]::Min($width, $height) * $fraction)
    $g.DrawImage((New-Icon 256 $zh.Glyph $zh.Top $zh.Bottom), [int](($width - $side) / 2), [int](($height - $side) / 2), $side, $side)
    $g.Dispose()
    $canvas.Save((Join-Path $ui $name), [Drawing.Imaging.ImageFormat]::Png)
}
Save-Canvas 'LockScreenLogo.scale-200.png' 48 48 1
Save-Canvas 'SplashScreen.scale-200.png' 1240 600 0.5
Save-Canvas 'Square150x150Logo.scale-200.png' 300 300 1
Save-Canvas 'Square44x44Logo.scale-200.png' 88 88 1
Save-Canvas 'Square44x44Logo.targetsize-24_altform-unplated.png' 24 24 1
Save-Canvas 'Square44x44Logo.targetsize-48_altform-lightunplated.png' 48 48 1
Save-Canvas 'StoreLogo.png' 50 50 1
Save-Canvas 'Wide310x150Logo.scale-200.png' 620 300 0.8
