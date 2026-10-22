# Packs the built tubeglass.exe and ffmpeg into dist\TubeGlass-<version>-windows-x86_64.zip (run on Windows).
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
Copy-Item packaging\THIRD_PARTY_NOTICES.md $dir
Copy-Item packaging\ffmpeg-LICENSE-GPLv3.txt $dir

# bundle ffmpeg: download the pinned build, check it, unpack it next to TubeGlass.exe
if (-not $env:SKIP_FFMPEG) {
    $row = (Get-Content packaging\ffmpeg-sources.tsv | Where-Object { $_ -match '^win32-x64\t' }) -split "`t"
    $gz = Join-Path $dir "ffmpeg.gz"
    Invoke-WebRequest -Uri $row[1] -OutFile $gz
    $got = (Get-FileHash $gz -Algorithm SHA256).Hash.ToLower()
    if ($got -ne $row[2]) { throw "ffmpeg checksum mismatch: expected $($row[2]), got $got" }
    $in = [System.IO.File]::OpenRead($gz)
    $out = [System.IO.File]::Create((Join-Path $dir "ffmpeg.exe"))
    $gzs = New-Object System.IO.Compression.GZipStream($in, [System.IO.Compression.CompressionMode]::Decompress)
    $gzs.CopyTo($out)
    $gzs.Dispose(); $out.Dispose(); $in.Dispose()
    Remove-Item $gz
}
@"
TubeGlass $version

Run TubeGlass.exe. Everything it needs is in this folder (ffmpeg.exe is
included; see THIRD_PARTY_NOTICES.md). Settings and downloads are kept in your
user folders; deleting this folder removes the program only.

https://github.com/flawlesscode254/TubeGlass
"@ | Set-Content -Encoding UTF8 (Join-Path $dir "README.txt")

$zip = "$dir.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $dir -DestinationPath $zip
Remove-Item -Recurse -Force $dir
Write-Host "Built $zip"
