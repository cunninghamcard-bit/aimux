#!/usr/bin/env bash
# Assert every dtolnay/rust-toolchain@<v> in .github/workflows/*.yml equals
# the channel pinned in rust-toolchain.toml. Prints file:line per mismatch.
set -euo pipefail
cd "$(dirname "$0")/.."
channel=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
if [ -z "$channel" ]; then
  echo "rust-toolchain.toml: no channel found" >&2
  exit 1
fi
status=0
found=0
while IFS=: read -r file line text; do
  found=$((found + 1))
  version=$(printf '%s' "$text" | sed -n 's/.*dtolnay\/rust-toolchain@\([^[:space:]#]*\).*/\1/p')
  if [ "$version" != "$channel" ]; then
    echo "$file:$line: dtolnay/rust-toolchain@$version does not match rust-toolchain.toml channel $channel" >&2
    status=1
  fi
done < <(grep -n 'dtolnay/rust-toolchain@' .github/workflows/*.yml)
if [ "$found" -eq 0 ]; then
  echo "no dtolnay/rust-toolchain steps found" >&2
  exit 1
fi
[ "$status" -eq 0 ] && echo "toolchain pin OK: $found dtolnay/rust-toolchain steps use $channel"
exit "$status"
