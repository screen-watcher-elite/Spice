#!/usr/bin/env bash
set -euo pipefail

PROJECT_ID="${1:?Usage: ./scripts/deploy_gcp.sh <GCP_PROJECT_ID> [REGION]}"
REGION="${2:-asia-south1}"
SERVICE_NAME="${SERVICE_NAME:-aegismesh}"
REPO_NAME="${REPO_NAME:-aegismesh}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

gcloud config set project "$PROJECT_ID" >/dev/null
gcloud services enable run.googleapis.com cloudbuild.googleapis.com artifactregistry.googleapis.com secretmanager.googleapis.com

if ! gcloud artifacts repositories describe "$REPO_NAME" --location "$REGION" >/dev/null 2>&1; then
  gcloud artifacts repositories create "$REPO_NAME" \
    --repository-format=docker \
    --location="$REGION" \
    --description="AegisMesh Fintech AI Firewall containers"
fi

if ! gcloud secrets describe aegismesh-gemini-api-key >/dev/null 2>&1 && [ -f ".env" ]; then
  KEY_VAL="$(grep -E '^GEMINI_API_KEY=' .env | head -n1 | cut -d'=' -f2- | tr -d '"' | tr -d "'" | xargs)"
  if [ -n "$KEY_VAL" ]; then
    gcloud secrets create aegismesh-gemini-api-key --replication-policy="user-managed" --locations="$REGION"
    printf '%s' "$KEY_VAL" | gcloud secrets versions add aegismesh-gemini-api-key --data-file=-
  fi
fi

if ! gcloud secrets describe aegismesh-admin-key >/dev/null 2>&1; then
  GEN_KEY="$(openssl rand -hex 20)"
  gcloud secrets create aegismesh-admin-key --replication-policy="user-managed" --locations="$REGION"
  printf '%s' "$GEN_KEY" | gcloud secrets versions add aegismesh-admin-key --data-file=-
fi

SHORT_SHA="$(git rev-parse --short HEAD 2>/dev/null || echo manual)"
gcloud builds submit \
  --config cloudbuild.yaml \
  --substitutions "_REGION=${REGION},_SERVICE_NAME=${SERVICE_NAME},_AR_REPO=${REPO_NAME},SHORT_SHA=${SHORT_SHA}" \
  .

gcloud run services describe "$SERVICE_NAME" --region "$REGION" --format="value(status.url)"
