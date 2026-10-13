#Requires -Version 5.1
# Install stack-stat from a checksum-verified GitHub Release.
$ErrorActionPreference = 'Stop'
$Repo = 'cam1se/stack-stat'
$BinaryName = 'stack-stat'
$Target = 'x86_64-pc-windows-msvc'
$InstallDir = $env:INSTALL_DIR
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\stack-stat' }
if (-not [System.IO.Path]::IsPathRooted($InstallDir)) { $InstallDir = Join-Path (Get-Location).Path $InstallDir }
$Tag = $env:STACK_STAT_VERSION
if (-not $Tag) {
    $Tag = gh release list --repo $Repo --limit 1000 --json 'tagName,isDraft' `
        --jq '[.[] | select(.isDraft == false) | .tagName | select(test("^v[0-9]+\\.[0-9]+\\.[0-9]+$"))] | sort_by(ltrimstr("v") | split(".") | map(tonumber)) | last // empty'
}
if ($Tag -notmatch '^v\d+\.\d+\.\d+$') { throw 'No stable release found. Set STACK_STAT_VERSION=vX.Y.Z.' }
$Version = $Tag -replace '^v', ''
$Archive = "${BinaryName}-${Version}-${Target}.zip"
$Work = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $Work | Out-Null
try {
    gh release download $Tag --repo $Repo --pattern $Archive --pattern 'checksums.sha256' --dir $Work --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Download failed' }
    $Line = Get-Content (Join-Path $Work 'checksums.sha256') | Where-Object { $_.EndsWith(" $Archive") } | Select-Object -First 1
    if (-not $Line) { throw "Checksum for $Archive not found" }
    $Expected = ($Line -split '\s+')[0].ToLower()
    $Actual = (Get-FileHash (Join-Path $Work $Archive) -Algorithm SHA256).Hash.ToLower()
    if ($Expected -ne $Actual) { throw "Checksum mismatch for $Archive" }
    Expand-Archive -Path (Join-Path $Work $Archive) -DestinationPath $Work
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $Work "${BinaryName}-${Version}-${Target}\${BinaryName}.exe") (Join-Path $InstallDir "${BinaryName}.exe") -Force
    $UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
    if ($UserPath.Split(';') -notcontains $InstallDir) {
        [Environment]::SetEnvironmentVariable('PATH', "$InstallDir;$UserPath", 'User')
        $env:PATH = "$InstallDir;$env:PATH"
    }
    & (Join-Path $InstallDir "${BinaryName}.exe") --version
} finally {
    Remove-Item $Work -Recurse -Force -ErrorAction SilentlyContinue
}
