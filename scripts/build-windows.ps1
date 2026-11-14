# Native Windows build of Lepton-GetThermal (x64, MSVC). Counterpart of build-windows.sh,
# which cross-compiles from Linux with cargo-xwin.
#   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1              -> portable zip
#   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Installer   -> + NSIS setup.exe
#   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Debug       -> debug exe, no zip
# Prereqs: VS 2022 Build Tools (C++ workload + Windows SDK), rustup (stable-x86_64-pc-windows-msvc),
# Node.js LTS, `npm install`. Staged runtime files in src-tauri\windows-libs\.
param([switch]$Installer, [switch]$Debug)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

# Low-RAM VMs (esp. without a pagefile): parallel rustc jobs run out of memory and fail with
# bogus errors (ICEs, "import resolution is stuck", 0xc0000409). The `windows` crate alone needs
# ~3 GB, so cap jobs at 1 per 4 GB of commit limit (RAM + pagefile). 5.7 GB RAM, no pagefile -> 1 job.
if (-not $env:CARGO_BUILD_JOBS) {
  $commitGB = (Get-CimInstance Win32_OperatingSystem).TotalVirtualMemorySize / 1MB
  $jobs = [Math]::Max(1, [Math]::Min([Environment]::ProcessorCount, [Math]::Floor($commitGB / 4)))
  if ($jobs -lt [Environment]::ProcessorCount) { $env:CARGO_BUILD_JOBS = "$jobs"; Write-Host "Commit limit $([Math]::Round($commitGB,1)) GB -> CARGO_BUILD_JOBS=$jobs" }
}
$Libs = 'src-tauri\windows-libs'
$Rel  = if ($Debug) { 'src-tauri\target\debug' } else { 'src-tauri\target\release' }

# 1) Runtime files (DirectML from ort's prebuilt ONNX Runtime, VC++ runtime, redist for the installer).
New-Item -ItemType Directory -Force $Libs | Out-Null
if (-not (Test-Path "$Libs\vc_redist.x64.exe")) {
  Invoke-WebRequest -UseBasicParsing https://aka.ms/vs/17/release/vc_redist.x64.exe -OutFile "$Libs\vc_redist.x64.exe"
}
if (-not (Test-Path "$Libs\MSVCP140.dll")) {
  $crt = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
  if (-not $crt) { throw "VC++ CRT redist folder not found; copy MSVCP140*.dll / VCRUNTIME140*.dll into $Libs" }
  foreach ($n in 'msvcp140','msvcp140_1','vcruntime140','vcruntime140_1') { Copy-Item "$($crt.FullName)\$n.dll" "$Libs\$($n.ToUpper()).dll" }
}

# 2) ort-sys's copy-dylibs step expects these directories to exist.
foreach ($p in "$Rel\build\examples","$Rel\build\deps","$Rel\examples","$Rel\deps") { New-Item -ItemType Directory -Force $p | Out-Null }

if (-not (Test-Path "$Libs\DirectML.dll")) {
  $dml = Get-ChildItem "$env:LOCALAPPDATA\ort.pyke.io\dfbin\x86_64-pc-windows-msvc\*\DirectML.dll" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $dml) {
    Push-Location src-tauri; try { cargo build --release -p ort-sys } finally { Pop-Location }
    $dml = Get-ChildItem "$env:LOCALAPPDATA\ort.pyke.io\dfbin\x86_64-pc-windows-msvc\*\DirectML.dll" -Recurse | Select-Object -First 1
  }
  Copy-Item $dml.FullName "$Libs\DirectML.dll"
}

# 3) Build.
if (-not (Test-Path node_modules)) { npm ci; if ($LASTEXITCODE) { throw 'npm ci failed' } }
$tauriArgs = @('run','tauri','build','--')
if ($Debug) { $tauriArgs += '--debug' }
if (-not $Installer) { $tauriArgs += '--no-bundle' } else { $tauriArgs += @('--bundles','nsis') }
& npm @tauriArgs
if ($LASTEXITCODE) { throw "tauri build failed ($LASTEXITCODE)" }
if ($Debug) { Write-Host "Debug exe: $Rel\lepton-getthermal.exe"; return }

# 4) Portable zip: exe + DLLs, nothing to install.
$ver  = (Get-Content src-tauri\tauri.conf.json -Raw | ConvertFrom-Json).version
$name = "Lepton-GetThermal_${ver}_x64-portable"
$out  = "$Rel\portable\$name"
if (Test-Path "$Rel\portable") { Remove-Item -Recurse -Force "$Rel\portable" }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "$Rel\lepton-getthermal.exe" "$out\Lepton-GetThermal.exe"
foreach ($n in 'DirectML.dll','MSVCP140.dll','MSVCP140_1.dll','VCRUNTIME140.dll','VCRUNTIME140_1.dll') { Copy-Item "$Libs\$n" $out }
Copy-Item packaging\windows\README-portable.txt "$out\README.txt"
Compress-Archive -Path $out -DestinationPath "$Rel\portable\$name.zip" -Force
Get-Item "$Rel\portable\$name.zip" | Select-Object FullName, Length
if ($Installer) { Get-ChildItem "$Rel\bundle\nsis" }
