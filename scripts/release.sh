#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
VERSION="${1:-}"
REGISTRY="${ONYX_REGISTRY:-ghcr.io/example}"

[[ "$VERSION" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+([+-][A-Za-z0-9.-]+)?$ ]] || {
  echo "usage: scripts/release.sh <vMAJOR.MINOR.PATCH>" >&2; exit 2;
}
VERSION="${VERSION#v}"
command -v cargo >/dev/null
command -v docker >/dev/null
command -v cosign >/dev/null
command -v gpg >/dev/null
command -v syft >/dev/null

scripts/ci-pipeline.sh
cargo generate-lockfile
mkdir -p "release/${VERSION}/binaries" "release/${VERSION}/sbom"
cargo build --locked --release -p api-server -p worker -p sync-agent -p migration-tool
for binary in api-server worker sync-agent migration-tool; do
  cp "target/release/${binary}" "release/${VERSION}/binaries/"
done
tar -C "release/${VERSION}/binaries" -czf "release/${VERSION}/onyx-${VERSION}-$(uname -s)-$(uname -m).tar.gz" .
gpg --batch --yes --armor --detach-sign "release/${VERSION}/onyx-${VERSION}-$(uname -s)-$(uname -m).tar.gz"

syft dir:. -o "spdx-json@2.3=release/${VERSION}/sbom/onyx-${VERSION}.spdx.json"
python3 - "release/${VERSION}/sbom/onyx-${VERSION}.spdx.json" <<'PYVERIFY'
import json, sys
path = sys.argv[1]
data = json.load(open(path, encoding="utf-8"))
if data.get("spdxVersion") != "SPDX-2.3":
    raise SystemExit(f"SBOM is not SPDX 2.3: {data.get('spdxVersion')!r}")
PYVERIFY

for service in api-server worker sync-agent migration-tool; do
  image="${REGISTRY}/onyx-${service}:${VERSION}"
  docker build -f "deploy/docker/${service}.Dockerfile" -t "$image" .
  if [[ "${ONYX_PUSH_RELEASE:-0}" == "1" ]]; then
    docker push "$image"
    cosign sign --yes "$image"
  fi
done

find "release/${VERSION}" -type f ! -name SHA256SUMS -print0 \
  | sort -z \
  | xargs -0 sha256sum > "release/${VERSION}/SHA256SUMS"
echo "release ${VERSION} assembled under release/${VERSION}"
