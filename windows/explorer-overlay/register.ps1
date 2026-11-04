#Requires -RunAsAdministrator
param([switch]$Uninstall)
$ErrorActionPreference = 'Stop'
if (![Environment]::Is64BitProcess) { throw 'Use 64-bit PowerShell.' }

$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$source = Join-Path $PSScriptRoot 'CloudloomExplorerOverlay.dll'
if (!(Test-Path -LiteralPath $source -PathType Leaf)) {
    $source = Join-Path $repo '.qa\explorer-overlay\CloudloomExplorerOverlay.dll'
}
$directory = Join-Path $env:ProgramFiles 'Cloudloom Explorer Overlay'
$dll = Join-Path $directory 'CloudloomExplorerOverlay.dll'
$classes = 'HKLM:\SOFTWARE\Classes\CLSID'
$overlays = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers'
$handlers = [ordered]@{
    'Cloudloom Sync Pending'  = '{7F0A4686-1A58-42F1-A104-178802E51341}'
    'Cloudloom Sync Complete' = '{7F0A4686-1A58-42F1-A104-178802E51342}'
}

foreach ($path in @($directory, $dll)) {
    if ((Test-Path -LiteralPath $path) -and
        ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'The installation path contains a link.'
    }
}
$ownedDll = $false
foreach ($handler in $handlers.GetEnumerator()) {
    $classKey = Join-Path $classes $handler.Value
    $serverKey = Join-Path $classKey 'InprocServer32'
    $overlayKey = Join-Path $overlays $handler.Key
    if ((Test-Path -LiteralPath $classKey) -and
        (!(Test-Path -LiteralPath $serverKey) -or (Get-Item -LiteralPath $serverKey).GetValue('') -ine $dll)) {
        throw 'A COM class is already registered to a different server.'
    }
    if (Test-Path -LiteralPath $serverKey) { $ownedDll = $true }
    if ((Test-Path -LiteralPath $overlayKey) -and (Get-Item -LiteralPath $overlayKey).GetValue('') -ine $handler.Value) {
        throw 'An overlay name is already registered to a different class.'
    }
}

if ($Uninstall) {
    foreach ($handler in $handlers.GetEnumerator()) {
        $classKey = Join-Path $classes $handler.Value
        foreach ($key in @((Join-Path $overlays $handler.Key), (Join-Path $classKey 'InprocServer32'), $classKey)) {
            if (Test-Path -LiteralPath $key) { Remove-Item -LiteralPath $key }
        }
    }
    if ((Test-Path -LiteralPath $dll) -and ($ownedDll -or (
        (Test-Path -LiteralPath $source) -and
        (Get-FileHash -LiteralPath $dll).Hash -eq (Get-FileHash -LiteralPath $source).Hash
    ))) {
        try { Remove-Item -LiteralPath $dll } catch {
            Write-Warning 'Sign out to release the DLL, then run this command again.'
        }
    }
} else {
    if (!(Test-Path -LiteralPath $source -PathType Leaf)) { throw 'Run build.ps1 -Test first.' }
    if (!(Test-Path -LiteralPath $directory -PathType Container)) {
        New-Item -ItemType Directory -Path $directory | Out-Null
    } elseif (!$ownedDll -or !(Test-Path -LiteralPath $dll -PathType Leaf)) {
        throw 'The installation directory already exists. Remove it if it is empty before installing.'
    }
    Copy-Item -LiteralPath $source -Destination $dll -Force
    foreach ($handler in $handlers.GetEnumerator()) {
        $classKey = Join-Path $classes $handler.Value
        $serverKey = Join-Path $classKey 'InprocServer32'
        $overlayKey = Join-Path $overlays $handler.Key
        if (!(Test-Path -LiteralPath $classKey)) { New-Item -Path $classKey | Out-Null }
        if (!(Test-Path -LiteralPath $serverKey)) { New-Item -Path $serverKey | Out-Null }
        if (!(Test-Path -LiteralPath $overlayKey)) { New-Item -Path $overlayKey | Out-Null }
        Set-Item -LiteralPath $serverKey -Value $dll
        Set-ItemProperty -LiteralPath $serverKey -Name ThreadingModel -Value Both
        Set-Item -LiteralPath $overlayKey -Value $handler.Value
    }
}
Write-Output 'Sign out and sign back in to reload Explorer overlay handlers.'
