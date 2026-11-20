$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "  AEGISMESH RELEASE BUILDER (.EXE + WINDOWS INSTALLER)" -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan

Write-Host "`n[1/4] Typechecking & building Svelte 5 frontend..." -ForegroundColor Yellow
Push-Location "$Root\frontend"
npm run check
if ($LASTEXITCODE -ne 0) { throw "svelte-check failed" }
npm run build
if ($LASTEXITCODE -ne 0) { throw "vite build failed" }
Pop-Location

Write-Host "`n[2/4] Compiling release binary aegismesh.exe..." -ForegroundColor Yellow
cargo build --release -p aegismesh-server
if ($LASTEXITCODE -ne 0) { throw "aegismesh-server release build failed" }

$DistDir = "$Root\dist"
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
$ServerExe = "$Root\target\release\aegismesh.exe"
$DistExe = "$DistDir\aegismesh.exe"
Copy-Item $ServerExe $DistExe -Force

Write-Host "`n[3/4] Compiling native Windows installer aegismesh-setup.exe..." -ForegroundColor Yellow
$env:AEGISMESH_BUNDLE_EXE = $DistExe
$env:AEGISMESH_BUNDLE_DB = "$Root\database\moneymax.db"
cargo build --release -p aegismesh-setup
if ($LASTEXITCODE -ne 0) { throw "aegismesh-setup release build failed" }
Remove-Item Env:\AEGISMESH_BUNDLE_EXE -ErrorAction SilentlyContinue
Remove-Item Env:\AEGISMESH_BUNDLE_DB -ErrorAction SilentlyContinue

Copy-Item "$Root\target\release\aegismesh-setup.exe" "$DistDir\aegismesh-setup.exe" -Force

Write-Host "`n[4/4] Packaging portable ZIP bundle..." -ForegroundColor Yellow
$PortableStage = "$DistDir\_portable_stage"
Remove-Item $PortableStage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path "$PortableStage\database" | Out-Null
Copy-Item $DistExe "$PortableStage\aegismesh.exe" -Force
Copy-Item "$Root\database\moneymax.db" "$PortableStage\database\moneymax.db" -Force
Copy-Item "$Root\aegismesh.toml.example" "$PortableStage\aegismesh.toml" -Force
if (Test-Path "$Root\README.md") {
    Copy-Item "$Root\README.md" "$PortableStage\README.md" -Force
}

$ZipPath = "$DistDir\AegisMesh-Portable.zip"
Remove-Item $ZipPath -Force -ErrorAction SilentlyContinue
Compress-Archive -Path "$PortableStage\*" -DestinationPath $ZipPath -Force
Remove-Item $PortableStage -Recurse -Force

Write-Host "`n==================================================================" -ForegroundColor Green
Write-Host "  BUILD COMPLETE!" -ForegroundColor Green
Get-ChildItem $DistDir | Format-Table Name, Length, LastWriteTime
Write-Host "==================================================================" -ForegroundColor Green
