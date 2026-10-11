#!/usr/bin/env bash
# Fetch the pinned AI SDK package sources into target/aisdk/<pkg>@<ver>/.
#
# The pinned versions come only from fixtures/aisdk/VERSIONS.json (override the
# file with AISDK_VERSIONS=<path>). Each value is either a version string or an
# object {"version": "...", "integrity": "sha512-..."}. Every tarball's sha512
# integrity is checked against VERSIONS.json when given, otherwise against
# `npm view <pkg>@<ver> dist.integrity`.
#
# Two phases: every package is fetched, verified and extracted into a staging
# directory first; only when all of them pass are the destinations replaced.
# Any failure (network, missing version, integrity or version mismatch)
# leaves target/aisdk exactly as it was. Running it again replaces each
# destination with identical content.
#
# Usage: bash scripts/fetch-aisdk.sh

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

die() {
    printf 'fetch-aisdk: %s\n' "$*" >&2
    exit 1
}

for tool in jq npm tar openssl base64; do
    command -v "$tool" >/dev/null 2>&1 || die "required tool not found: $tool"
done

versions_file="${AISDK_VERSIONS:-fixtures/aisdk/VERSIONS.json}"
[[ -f "$versions_file" ]] || die "versions file not found: $versions_file"
jq -e 'type == "object" and length > 0' "$versions_file" >/dev/null ||
    die "$versions_file must be a non-empty JSON object"

dest_root="target/aisdk"
mkdir -p "$dest_root"
work="$(mktemp -d "$dest_root/.work.XXXXXX")"
trap 'rm -rf "$work"' EXIT

# pkg<TAB>version<TAB>integrity (integrity empty when not pinned)
entries="$(jq -r '
    to_entries[] |
    if (.value | type) == "string" then [.key, .value, ""]
    else [.key, (.value.version // ""), (.value.integrity // "")] end |
    @tsv' "$versions_file")"

specs=()
while IFS=$'\t' read -r pkg ver integrity; do
    [[ -n "$pkg" && -n "$ver" ]] || die "bad entry in $versions_file: '$pkg' '$ver'"
    spec="$pkg@$ver"
    echo "fetch-aisdk: $spec"

    if [[ -z "$integrity" ]]; then
        integrity="$(npm view "$spec" dist.integrity 2>/dev/null)" ||
            die "npm view failed for $spec (network error or no such version)"
        [[ -n "$integrity" ]] || die "no dist.integrity for $spec"
    fi
    [[ "$integrity" == sha512-* ]] || die "$spec: unsupported integrity '$integrity' (need sha512)"

    pack_dir="$work/pack"
    rm -rf "$pack_dir" && mkdir -p "$pack_dir"
    tarball_name="$(npm pack "$spec" --silent --pack-destination "$pack_dir" 2>/dev/null | tail -n1)" ||
        die "npm pack failed for $spec (network error or no such version)"
    tarball="$pack_dir/$tarball_name"
    [[ -f "$tarball" ]] || die "npm pack produced no tarball for $spec"

    actual="sha512-$(openssl dgst -sha512 -binary "$tarball" | base64 -w0)"
    [[ "$actual" == "$integrity" ]] ||
        die "$spec: integrity mismatch (expected $integrity, got $actual)"

    staging="$work/stage/$spec"
    unpack="$work/unpack"
    rm -rf "$unpack" && mkdir -p "$unpack"
    tar -xzf "$tarball" -C "$unpack" --no-same-owner --no-same-permissions
    [[ -f "$unpack/package/package.json" ]] || die "$spec: tarball has no package/package.json"

    got="$(jq -r '.name + "@" + .version' "$unpack/package/package.json")"
    [[ "$got" == "$spec" ]] || die "version mismatch: wanted $spec, tarball is $got"

    mkdir -p "$(dirname "$staging")"
    mv "$unpack/package" "$staging"
    specs+=("$spec")
done <<<"$entries"

# Phase 2: every package verified. Move the old trees aside, place the new
# ones, and put the old ones back if a move fails.
placed=()
restore() {
    local spec
    for spec in "${placed[@]}"; do
        rm -rf "${dest_root:?}/$spec"
    done
    for spec in "${specs[@]}"; do
        if [[ -e "$work/old/$spec" ]]; then
            mkdir -p "$(dirname "$dest_root/$spec")"
            mv "$work/old/$spec" "$dest_root/$spec"
        fi
    done
}
for spec in "${specs[@]}"; do
    if [[ -e "$dest_root/$spec" ]]; then
        mkdir -p "$(dirname "$work/old/$spec")"
        mv "$dest_root/$spec" "$work/old/$spec" || { restore; die "could not move $dest_root/$spec aside"; }
    fi
done
for spec in "${specs[@]}"; do
    mkdir -p "$(dirname "$dest_root/$spec")"
    mv "$work/stage/$spec" "$dest_root/$spec" || { restore; die "could not place $spec"; }
    placed+=("$spec")
done

echo "fetch-aisdk: done ($(jq length "$versions_file") packages in $dest_root)"
