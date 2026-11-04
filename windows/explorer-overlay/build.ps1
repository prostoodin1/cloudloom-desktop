param([switch]$Test)
$ErrorActionPreference = 'Stop'
$source = $PSScriptRoot
$repo = Split-Path (Split-Path $source -Parent) -Parent
$output = Join-Path $repo '.qa\explorer-overlay'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw 'Visual Studio C++ Build Tools are required' }
& (Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
Push-Location $output
try {
    python (Join-Path $source 'icons.py') $output
    if ($LASTEXITCODE) { throw 'Icon generation failed' }
    rc.exe /nologo /fo overlay.res (Join-Path $source 'overlay.rc')
    if ($LASTEXITCODE) { throw 'Resource compilation failed' }
    cl.exe /nologo /std:c++17 /W4 /EHsc /O2 /MT /LD (Join-Path $source 'overlay.cpp') overlay.res advapi32.lib uuid.lib /link /OUT:CloudloomExplorerOverlay.dll ("/DEF:" + (Join-Path $source 'overlay.def'))
    if ($LASTEXITCODE) { throw 'Overlay compilation failed' }
    if ($Test) {
        cl.exe /nologo /std:c++17 /W4 /EHsc /MT (Join-Path $source 'smoke.cpp') advapi32.lib shell32.lib user32.lib uuid.lib /link /OUT:overlay-smoke.exe
        if ($LASTEXITCODE) { throw 'Test compilation failed' }
        & (Join-Path $output 'overlay-smoke.exe') (Join-Path $output 'CloudloomExplorerOverlay.dll')
        if ($LASTEXITCODE) { throw 'Overlay test failed' }
    }
} finally {
    Pop-Location
}
