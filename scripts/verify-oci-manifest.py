#!/usr/bin/env python3
"""Verify the immutable one-layer v0.1.0 provider OCI manifest."""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
import sys

VERSION = "0.1.0"
SOURCE = "https://github.com/dekopon-agents/dekopon-provider-memory-chat"
RELEASE = f"{SOURCE}/releases/tag/v{VERSION}"
ARTIFACT_TYPE = "application/vnd.dekopon.provider.v1+wasm"


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def main() -> None:
    if len(sys.argv) != 5:
        fail("usage: verify-oci-manifest.py MANIFEST COMPONENT RUN REVISION")
    manifest_path, component_path, run, revision = sys.argv[1:]
    if not re.fullmatch(r"[0-9]+:[0-9]+", run):
        fail("invalid run marker")
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        fail("invalid Git revision")

    manifest = json.loads(pathlib.Path(manifest_path).read_text(encoding="utf-8"))
    component = pathlib.Path(component_path)
    component_bytes = component.read_bytes()
    expected_digest = "sha256:" + hashlib.sha256(component_bytes).hexdigest()

    if manifest.get("schemaVersion") != 2:
        fail("OCI schemaVersion must be 2")
    if manifest.get("artifactType") != ARTIFACT_TYPE:
        fail("OCI artifact type mismatch")
    config = manifest.get("config", {})
    if config.get("size") not in {0, 2}:
        fail("OCI config must be empty")

    expected_annotations = {
        "org.opencontainers.image.source": SOURCE,
        "org.opencontainers.image.version": VERSION,
        "org.opencontainers.image.revision": revision,
        "org.opencontainers.image.licenses": "MIT OR Apache-2.0",
        "org.dekopon.release.run": run,
        "org.dekopon.release.url": RELEASE,
        "org.dekopon.provider.capability": "memory.chat.record,memory.chat.recent,memory.chat.search",
    }
    annotations = manifest.get("annotations", {})
    for key, expected in expected_annotations.items():
        if annotations.get(key) != expected:
            fail(f"OCI annotation {key!r} mismatch")
    forbidden = json.dumps(manifest, sort_keys=True).lower()
    for spelling in ("latest", "staging", ":temp", ":tmp"):
        if spelling in forbidden:
            fail(f"OCI manifest contains forbidden mutable spelling {spelling!r}")

    layers = manifest.get("layers", [])
    if len(layers) != 1:
        fail("provider OCI manifest must contain exactly one layer")
    layer = layers[0]
    if layer.get("mediaType") != "application/wasm":
        fail("provider layer media type is not application/wasm")
    if layer.get("digest") != expected_digest:
        fail("provider layer digest does not match release bytes")
    if layer.get("size") != len(component_bytes):
        fail("provider layer size does not match release bytes")
    if layer.get("annotations", {}).get("org.opencontainers.image.title") != component.name:
        fail("provider layer title does not match release asset name")


if __name__ == "__main__":
    main()
