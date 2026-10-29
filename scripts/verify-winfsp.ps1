# Disposable GitHub Windows runner ONLY. Never install/uninstall a user's driver in tests.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'WinFsp installation test is restricted to a disposable GitHub Actions runner' }
$projectRoot = Split-Path -Parent $PSScriptRoot
$msi = Join-Path $projectRoot 'src-tauri/resources/winfsp/winfsp-2.1.25156.msi'
if ((Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash.ToLowerInvariant() -ne '073a70e00f77423e34bed98b86e600def93393ba5822204fac57a29324db9f7a') { throw 'WinFsp hash mismatch' }
if ((Get-AuthenticodeSignature -LiteralPath $msi).Status -ne 'Valid') { throw 'WinFsp signature invalid' }
$installed = Test-Path -LiteralPath 'C:/Program Files (x86)/WinFsp/bin/winfsp-x64.dll'
if (!$installed) {
    $lockedMsi = [System.IO.File]::Open($msi, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    try { $process = Start-Process -FilePath "$env:SystemRoot/System32/msiexec.exe" -ArgumentList @('/i', ('"' + $msi + '"'), '/qn', '/norestart') -WindowStyle Hidden -Wait -PassThru }
    finally { $lockedMsi.Dispose() }
    if ($process.ExitCode -notin @(0,3010)) { throw "WinFsp installation failed: $($process.ExitCode)" }
}
$env:CLOUDLOOM_TEST_MOUNTS='1'
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib real_windows_mount_read_write_readonly_and_appearance -- --nocapture
if ($LASTEXITCODE) { throw 'Actual WinFsp filesystem test failed' }
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib explorer_delivery_bridge_real_rclone_roundtrip -- --nocapture
if ($LASTEXITCODE) { throw 'Actual Delivery API WinFsp filesystem test failed' }
Write-Output 'Bundled WinFsp verified: signed installer, native mount, IO, read-only mode, icon, restart recovery, Delivery API bridge.'
