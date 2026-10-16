# Saves a picture of the whole Windows screen as a PNG file, for the dialog pictures of the build checks
# (.github/workflows/build.yml, Dialog layout check).
#
# Usage: powershell -NoProfile -File tools/ci/screenshot.ps1 <file.png>
param([Parameter(Mandatory = $true)][string]$Path)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bitmap = New-Object System.Drawing.Bitmap $screen.Width, $screen.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $bitmap.Size)
$bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
