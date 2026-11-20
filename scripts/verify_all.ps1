$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

Write-Host "[1/5] Clippy static analysis (-D warnings)..." -ForegroundColor Yellow
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "Clippy failed" }

Write-Host "[2/5] Running Rust workspace test suite..." -ForegroundColor Yellow
cargo test --workspace
if ($LASTEXITCODE -ne 0) { throw "Workspace tests failed" }

Write-Host "[3/5] Running 500-concurrency Red-Team Stress Suite..." -ForegroundColor Yellow
cargo run --release --bin redteam_stress
if ($LASTEXITCODE -ne 0) { throw "Redteam stress failed" }

Write-Host "[4/5] Running Inline Latency & Throughput Benchmark..." -ForegroundColor Yellow
cargo run --release --bin benchmark
if ($LASTEXITCODE -ne 0) { throw "Benchmark failed" }

Write-Host "[5/5] Verifying Svelte 5 frontend check & build..." -ForegroundColor Yellow
Push-Location "$Root\frontend"
npm run check
if ($LASTEXITCODE -ne 0) { throw "Frontend check failed" }
npm run build
if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }
Pop-Location

Write-Host "`nALL VERIFICATION GATES PASSED 100%!" -ForegroundColor Green
