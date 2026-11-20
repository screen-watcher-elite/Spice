param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectId,
    [string]$Region = "asia-south1",
    [string]$ServiceName = "aegismesh",
    [string]$RepoName = "aegismesh"
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Root

if (-not (Get-Command gcloud -ErrorAction SilentlyContinue)) {
    throw "Google Cloud CLI ('gcloud') is not installed or not in PATH. Install from https://cloud.google.com/sdk/docs/install"
}

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "  AEGISMESH -> GOOGLE CLOUD RUN DEPLOYMENT ($Region)" -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "  Project : $ProjectId"
Write-Host "  Region  : $Region (Indian Fintech DPDP Residency)"
Write-Host "  Service : $ServiceName"
Write-Host ""

gcloud config set project $ProjectId | Out-Null

Write-Host "[1/4] Enabling Cloud Run, Cloud Build, Artifact Registry & Secret Manager..." -ForegroundColor Yellow
gcloud services enable run.googleapis.com cloudbuild.googleapis.com artifactregistry.googleapis.com secretmanager.googleapis.com

Write-Host "[2/4] Ensuring Artifact Registry repository exists..." -ForegroundColor Yellow
$repoCheck = gcloud artifacts repositories describe $RepoName --location $Region --format="value(name)" 2>$null
if (-not $repoCheck) {
    gcloud artifacts repositories create $RepoName --repository-format=docker --location=$Region --description="AegisMesh Fintech AI Firewall containers"
}

Write-Host "[3/4] Checking Secret Manager bindings (aegismesh-gemini-api-key, aegismesh-admin-key)..." -ForegroundColor Yellow
$geminiSecretOk = gcloud secrets describe aegismesh-gemini-api-key --format="value(name)" 2>$null
if (-not $geminiSecretOk -and (Test-Path ".env")) {
    $line = Get-Content ".env" | Where-Object { $_ -match '^\s*GEMINI_API_KEY\s*=' } | Select-Object -First 1
    if ($line) {
        $keyVal = ($line -split '=', 2)[1].Trim().Trim('"').Trim("'")
        gcloud secrets create aegismesh-gemini-api-key --replication-policy="user-managed" --locations=$Region
        $keyVal | gcloud secrets versions add aegismesh-gemini-api-key --data-file=-
        Write-Host "  [OK] Provisioned aegismesh-gemini-api-key in $Region Secret Manager" -ForegroundColor Green
    }
}

$adminSecretOk = gcloud secrets describe aegismesh-admin-key --format="value(name)" 2>$null
if (-not $adminSecretOk) {
    $genKey = -join ((48..57) + (65..90) + (97..122) | Get-Random -Count 28 | ForEach-Object { [char]$_ })
    gcloud secrets create aegismesh-admin-key --replication-policy="user-managed" --locations=$Region
    $genKey | gcloud secrets versions add aegismesh-admin-key --data-file=-
    Write-Host "  [OK] Generated & stored random Admin Key in Secret Manager (aegismesh-admin-key)" -ForegroundColor Green
}

Write-Host "[4/4] Building container via Cloud Build & deploying to Cloud Run..." -ForegroundColor Yellow
$shortSha = (git rev-parse --short HEAD 2>$null)
if (-not $shortSha) { $shortSha = "manual" }
gcloud builds submit --config cloudbuild.yaml --substitutions "_REGION=$Region,_SERVICE_NAME=$ServiceName,_AR_REPO=$RepoName,SHORT_SHA=$shortSha" .

$url = gcloud run services describe $ServiceName --region $Region --format="value(status.url)"
Write-Host ""
Write-Host "==================================================================" -ForegroundColor Green
Write-Host "  DEPLOYED LIVE ON GOOGLE CLOUD RUN: $url" -ForegroundColor Green
Write-Host "==================================================================" -ForegroundColor Green
