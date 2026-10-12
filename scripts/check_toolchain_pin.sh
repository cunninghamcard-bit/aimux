#!/usr/bin/env bash
# Assert every dtolnay/rust-toolchain reference in .github/**/*.yml and
# .github/**/*.yaml (workflows and composite actions) is a version equal to
# the channel pinned in rust-toolchain.toml, and that no `toolchain:` key
# (block or flow style) overrides it. Prints file:line per failure.
set -euo pipefail
cd "$(dirname "$0")/.."
channel=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
if [ -z "$channel" ]; then
  echo "rust-toolchain.toml: no channel found" >&2
  exit 1
fi
mapfile -t files < <(find .github -type f \( -name '*.yml' -o -name '*.yaml' \) | sort)
if [ "${#files[@]}" -eq 0 ]; then
  echo "no .github/**/*.yml or *.yaml files found" >&2
  exit 1
fi
version_re='^[0-9]+\.[0-9]+(\.[0-9]+)?$'
status=0
found=0
while IFS=: read -r file line text; do
  found=$((found + 1))
  ref=$(printf '%s' "$text" | sed -n 's/.*dtolnay\/rust-toolchain@\([^[:space:]#"'"'"',}]*\).*/\1/Ip')
  if [ -z "$ref" ]; then
    echo "$file:$line: dtolnay/rust-toolchain without @$channel" >&2
    status=1
  elif ! [[ "$ref" =~ $version_re ]]; then
    echo "$file:$line: dtolnay/rust-toolchain@$ref is not a version (expected @$channel)" >&2
    status=1
  elif [ "$ref" != "$channel" ]; then
    echo "$file:$line: dtolnay/rust-toolchain@$ref does not match rust-toolchain.toml channel $channel" >&2
    status=1
  fi
done < <(grep -inH 'dtolnay/rust-toolchain' "${files[@]}" || true)
if [ "$found" -eq 0 ]; then
  echo "no dtolnay/rust-toolchain@ steps found in .github/**/*.y{a,}ml" >&2
  exit 1
fi
# Any `toolchain:` key: block style, flow style (`with: { toolchain: x }`),
# quoted or not, anywhere on the line.
while IFS=: read -r file line _; do
  echo "$file:$line: rust-toolchain 'toolchain:' input overrides the pin; use dtolnay/rust-toolchain@$channel" >&2
  status=1
done < <(grep -inHE '(^|[[:space:]{,])["'"'"']?toolchain["'"'"']?[[:space:]]*:' "${files[@]}" || true)
[ "$status" -eq 0 ] && echo "toolchain pin OK: $found dtolnay/rust-toolchain steps use $channel"
exit "$status"
