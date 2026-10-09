param([string]$ResourceDirectory = '')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$resourceFolder = if ($ResourceDirectory) { $ResourceDirectory } else { Join-Path $projectRoot 'src-tauri\resources' }
$enginePath = Join-Path $resourceFolder 'rclone.exe'
if (Test-Path -LiteralPath $enginePath) {
    $version = & $enginePath version
    if ($LASTEXITCODE -eq 0 -and $version[0] -eq 'rclone v1.75.2') { exit 0 }
    throw 'Unexpected rclone binary. Remove src-tauri/resources/rclone.exe and run prepare:engine again.'
}
$temporaryFolder = Join-Path ([System.IO.Path]::GetTempPath()) ('cloudloom-engine-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporaryFolder | Out-Null
$archivePath = Join-Path $temporaryFolder 'rclone.zip'
Invoke-WebRequest -Uri 'https://downloads.rclone.org/v1.75.2/rclone-v1.75.2-windows-amd64.zip' -OutFile $archivePath
$expectedHash = '8a9db2bad1dc79a65b3348fd857069598e2aab7cf4d118b77b1df23055aff818'
# .NET primitives also work when Windows PowerShell inherits a PS7 module path on CI.
$hashAlgorithm = [System.Security.Cryptography.SHA256]::Create()
$archiveStream = [System.IO.File]::OpenRead($archivePath)
try { $archiveHash = [System.BitConverter]::ToString($hashAlgorithm.ComputeHash($archiveStream)).Replace('-', '').ToLowerInvariant() }
finally { $archiveStream.Dispose(); $hashAlgorithm.Dispose() }
if ($archiveHash -ne $expectedHash) { throw 'rclone archive checksum mismatch' }
[System.Reflection.Assembly]::LoadWithPartialName('System.IO.Compression.FileSystem') | Out-Null
[System.IO.Compression.ZipFile]::ExtractToDirectory($archivePath, $temporaryFolder)
New-Item -ItemType Directory -Path $resourceFolder -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $temporaryFolder 'rclone-v1.75.2-windows-amd64\rclone.exe') -Destination $enginePath
Write-Output 'Verified rclone v1.75.2 is ready.'
