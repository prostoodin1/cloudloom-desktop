param([string]$InstallerPath = '')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (!$InstallerPath) {
    $installers = @(Get-ChildItem -LiteralPath (Join-Path $projectRoot 'src-tauri/target/release/bundle/nsis') -Filter '*.exe')
    if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer' }
    $InstallerPath = $installers[0].FullName
}
# Run on a clean CI account: installation touches the current user's app registration.
$testDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ('Cloudloom-install-test-' + [guid]::NewGuid().ToString('N'))
$installer = Start-Process -FilePath $InstallerPath -ArgumentList @('/S', "/D=$testDirectory") -WindowStyle Hidden -Wait -PassThru
if ($installer.ExitCode -ne 0) { throw "Installer failed: $($installer.ExitCode)" }
$executable = Join-Path $testDirectory 'cloudloom.exe'
$engine = Join-Path $testDirectory 'resources/rclone.exe'
foreach ($path in @($executable, $engine, (Join-Path $testDirectory 'resources/RCLONE-LICENSE.txt'), (Join-Path $testDirectory 'resources/CLOUDLOOM-LICENSE.txt'), (Join-Path $testDirectory 'resources/winfsp/License.txt'), (Join-Path $testDirectory 'resources/winfsp/winfsp-2.1.25156.msi'))) {
    if (!(Test-Path -LiteralPath $path)) { throw "Missing installed resource: $path" }
}
if ((Get-FileHash -LiteralPath (Join-Path $testDirectory 'resources/winfsp/winfsp-2.1.25156.msi') -Algorithm SHA256).Hash.ToLowerInvariant() -ne '073a70e00f77423e34bed98b86e600def93393ba5822204fac57a29324db9f7a') { throw 'Wrong packaged WinFsp installer' }
$version = & $engine version
if ($LASTEXITCODE -ne 0 -or $version[0] -ne 'rclone v1.75.2' -or $version -notcontains '- os/arch: amd64') { throw 'Wrong bundled Windows engine' }
$app = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
try {
    Start-Sleep -Seconds 12
    $app.Refresh()
    if ($app.HasExited) { throw "Installed app exited during startup: $($app.ExitCode)" }
    Write-Output 'Installer, packaged resources, native engine and application startup passed.'
} finally {
    if (!$app.HasExited) { Stop-Process -Id $app.Id -Force }
}
$uninstallerPath = Join-Path $testDirectory 'uninstall.exe'
if (!(Test-Path -LiteralPath $uninstallerPath)) { throw 'Missing uninstaller' }
# NSIS _?= runs the uninstaller in place so Wait observes the actual uninstall.
$uninstaller = Start-Process -FilePath $uninstallerPath -ArgumentList @('/S', "_?=$testDirectory") -WindowStyle Hidden -Wait -PassThru
if ($uninstaller.ExitCode -ne 0 -or (Test-Path -LiteralPath $executable)) { throw 'Uninstall verification failed' }
Write-Output 'Uninstall passed. Windows publisher signing is not configured.'
