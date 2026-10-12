#!/usr/bin/env bash
# Assert every dtolnay/rust-toolchain@<v> in .github/workflows/*.yml is a
# version equal to the channel pinned in rust-toolchain.toml, and that no
# step overrides it with a `toolchain:` input. Prints file:line per failure.
set -euo pipefail
cd "$(dirname "$0")/.."
channel=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
if [ -z "$channel" ]; then
  echo "rust-toolchain.toml: no channel found" >&2
  exit 1
fi
version_re='^[0-9]+\.[0-9]+(\.[0-9]+)?$'
status=0
found=0
while IFS=: read -r file line text; do
  found=$((found + 1))
  ref=$(printf '%s' "$text" | sed -n 's/.*dtolnay\/rust-toolchain@\([^[:space:]#]*\).*/\1/p')
  if ! [[ "$ref" =~ $version_re ]]; then
    echo "$file:$line: dtolnay/rust-toolchain@$ref is not a version (expected @$channel)" >&2
    status=1
  elif [ "$ref" != "$channel" ]; then
    echo "$file:$line: dtolnay/rust-toolchain@$ref does not match rust-toolchain.toml channel $channel" >&2
    status=1
  fi
done < <(grep -n 'dtolnay/rust-toolchain@' .github/workflows/*.yml || true)
if [ "$found" -eq 0 ]; then
  echo "no dtolnay/rust-toolchain@ steps found in .github/workflows/*.yml" >&2
  exit 1
fi
while IFS=: read -r file line _; do
  echo "$file:$line: rust-toolchain 'toolchain:' input overrides the pin; use dtolnay/rust-toolchain@$channel" >&2
  status=1
done < <(grep -nE '^[[:space:]]*toolchain:' .github/workflows/*.yml || true)
[ "$status" -eq 0 ] && echo "toolchain pin OK: $found dtolnay/rust-toolchain steps use $channel"
exit "$status"
