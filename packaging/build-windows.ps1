# Packs the built tubeglass.exe into dist\TubeGlass-<version>-windows-x86_64.zip (run on Windows).
#   cargo build --release --locked
#   .\packaging\build-windows.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = ((Select-String -Path Cargo.toml -Pattern '^version').Line -split '"')[1]
$name = "TubeGlass-$version-windows-x86_64"
$dir = Join-Path "dist" $name
if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
New-Item -ItemType Directory -Force $dir | Out-Null

Copy-Item "target\release\tubeglass.exe" (Join-Path $dir "TubeGlass.exe")
Copy-Item LICENSE $dir
@"
TubeGlass $version

Run TubeGlass.exe. TubeGlass needs ffmpeg to play and download:

    winget install ffmpeg

then restart TubeGlass. Settings and downloads are kept in your user folders;
deleting this folder removes the program only.

https://github.com/flawlesscode254/TubeGlass
"@ | Set-Content -Encoding UTF8 (Join-Path $dir "README.txt")

$zip = "$dir.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $dir -DestinationPath $zip
Remove-Item -Recurse -Force $dir
Write-Host "Built $zip"
