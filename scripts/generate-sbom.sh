#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
output=${1:?usage: generate-sbom.sh OUTPUT.json}
[[ "$(cargo cyclonedx --version)" == 'cargo-cyclonedx-cyclonedx 0.5.9' ]]
raw="$root/memory-chat-provider.raw.json"; trap 'rm -f "$raw"' EXIT
(cd "$root" && SOURCE_DATE_EPOCH=0 cargo cyclonedx --format json --spec-version 1.5 --target wasm32-unknown-unknown --override-filename memory-chat-provider.raw)
mkdir -p "$(dirname "$output")"
python3 - "$raw" "$output" "$root" <<'PY'
import json, pathlib, sys
raw, output, root = map(pathlib.Path, sys.argv[1:])
document = json.loads(raw.read_text())
if document.get("bomFormat") != "CycloneDX" or document.get("specVersion") != "1.5":
    raise SystemExit("unexpected CycloneDX version")
if document["metadata"]["timestamp"] != "1970-01-01T00:00:00.000000000Z":
    raise SystemExit("SBOM timestamp is not deterministic")
names = {(item["name"], item["version"]) for item in document.get("components", [])}
for expected in (("dekopon-provider-sdk", "0.13.0"), ("dekopon-provider-storage", "0.13.0"), ("serde", "1.0.229"), ("serde_json", "1.0.151"), ("wit-bindgen", "0.62.0")):
    if expected not in names:
        raise SystemExit(f"SBOM omits {expected}")
encoded = json.dumps(document, indent=2, sort_keys=True, ensure_ascii=False).replace(str(root), "/dekopon/source") + "\n"
if str(root) in encoded:
    raise SystemExit("SBOM retained local source path")
output.write_text(encoded, encoding="utf-8", newline="\n")
PY
printf 'generated deterministic CycloneDX 1.5 SBOM %s\n' "$output"
