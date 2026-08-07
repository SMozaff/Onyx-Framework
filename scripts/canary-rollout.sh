#!/usr/bin/env bash
set -euo pipefail
NAMESPACE=onyx
RELEASE=${1:-onyx-api}
CHART=${2:-deploy/helm/onyx-api}
VALUES=${3:-deploy/helm/onyx-api/values-production.yaml}
helm upgrade --install "$RELEASE" "$CHART" --namespace "$NAMESPACE" --create-namespace -f "$VALUES" --wait=false
kubectl argo rollouts get rollout "$RELEASE" -n "$NAMESPACE" --watch
