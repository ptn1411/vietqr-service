Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 180, 50
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$g.Clear([System.Drawing.Color]::White)

$penBlue = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(2, 132, 199), [float]3)
$brushDarkBlue = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30, 64, 175))
$brushCyan = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(14, 165, 233))

# Draw emblem at (6, 8) size 34x34
$g.FillEllipse($brushDarkBlue, 6, 8, 34, 34)
$g.FillEllipse($brushCyan, 14, 16, 18, 18)
$g.FillEllipse([System.Drawing.Brushes]::White, 19, 21, 8, 8)

# Text: PTN
$font = [System.Drawing.Font]::new('Arial', [float]24, [System.Drawing.FontStyle]::Bold)
$brushText = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30, 64, 175))
$g.DrawString('PTN', $font, $brushText, [float]48, [float]6)

$bmp.Save('vietqr-service\assets\brand\ptn_logo.png')
$g.Dispose()
$bmp.Dispose()
Write-Host 'Created ptn_logo.png successfully'
