# Package a Nuclear Sunset fork release in the shape the launcher's updater installs
# (crates/nvf-launcher/src/fork.rs, update.rs): NVForever-<tag>-x86_64-pc-windows-msvc.zip
# with one top folder of the same name, and SHA256SUMS.txt beside it.
#
#   tools\nuclear-sunset\package.ps1 -Tag ns-2026.11.27 -EngineTarget <dir> -RootTarget <dir> -Out <dir>
#
# The tag's date must equal fork::BUILD (the launcher compares the two). Builds nothing:
# build nvforever + nvf-viewer (engine) and fnvtool + nvforever-launcher (root) in release first.
# Packs no game data: our programs, the schemas, lang, the licences and the docs.
param(
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][string]$EngineTarget,
    [Parameter(Mandatory)][string]$RootTarget,
    [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'
$Src = (Resolve-Path "$PSScriptRoot\..\..").Path
$build = (Select-String -Path "$Src\crates\nvf-launcher\src\fork.rs" -Pattern 'pub const BUILD: &str = "([^"]+)"').Matches[0].Groups[1].Value
if ($Tag -ne "ns-$build") { throw "tag $Tag does not match fork::BUILD $build (raise BUILD first)" }

$name = "NVForever-$Tag-x86_64-pc-windows-msvc"
$dist = Join-Path $Out $name
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $dist | Out-Null
foreach ($f in @("$RootTarget\release\fnvtool.exe", "$RootTarget\release\nvforever-launcher.exe",
                 "$EngineTarget\release\nvforever.exe", "$EngineTarget\release\nvf-viewer.exe")) {
    if (-not (Test-Path $f)) { throw "missing $f" }
    Copy-Item $f $dist
}
Copy-Item -Recurse "$Src\schemas" "$dist\schemas"
Copy-Item -Recurse "$Src\lang" "$dist\lang"
foreach ($f in 'README.md', 'LICENSE', 'LEGAL.md', 'NUCLEAR-SUNSET.md', 'tools\release\README-release.md',
               'tools\release\nvforever.cmd', 'tools\release\nvf-viewer.cmd', 'tools\nuclear-sunset\Play Nuclear Sunset.cmd') {
    Copy-Item "$Src\$f" $dist
}
New-Item -ItemType Directory -Force "$dist\docs\nuclear-sunset" | Out-Null
Copy-Item "$Src\docs\nuclear-sunset\*.md" "$dist\docs\nuclear-sunset"
# The licence list asks cargo; the owner's PC keeps a portable toolchain off PATH.
$tools = 'D:\ai stuff\tools'
if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path "$tools\rust\cargo\bin\cargo.exe")) {
    $env:CARGO_HOME = "$tools\rust\cargo"; $env:RUSTUP_HOME = "$tools\rust\rustup"; $env:PATH = "$tools\rust\cargo\bin;$env:PATH"
}
$py = if (Test-Path "$tools\python\python.exe") { "$tools\python\python.exe" } else { 'python' }
$ErrorActionPreference = 'Continue'
& $py "$Src\tools\release\third_party_licenses.py" --target x86_64-pc-windows-msvc --out "$dist\THIRD-PARTY-LICENSES.txt" 2>&1 | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'third_party_licenses.py failed' }
$ErrorActionPreference = 'Stop'

$zip = Join-Path $Out "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
# Windows' own bsdtar: `/` between parts (Windows PowerShell's Compress-Archive writes `\`).
& "$env:SystemRoot\System32\tar.exe" -a -c -f $zip -C $Out $name
if ($LASTEXITCODE -ne 0) { throw 'tar failed' }
$hash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
"$hash  $name.zip" | Set-Content -Encoding ascii (Join-Path $Out 'SHA256SUMS.txt')
"packed $zip"
"sha256 $hash"
