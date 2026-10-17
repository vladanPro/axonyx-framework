#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if (( $# > 1 )) || { (( $# == 1 )) && [[ "$1" != --check ]]; }; then
  printf 'Usage: bash scripts/format-templates.sh [--check]\n' >&2
  exit 2
fi
formatter=${AXONYX_FORMATTER:-$(git config --get axonyx.formatterPath || true)}
command=(cargo run --quiet -p cargo-axonyx --bin cargo-ax --)
if [[ -n "$formatter" ]]; then command=("$formatter"); fi
failed=0
count=0
temporary=''
trap 'if [[ -n "$temporary" ]]; then rm -f -- "$temporary"; fi' EXIT
while IFS= read -r -d '' file; do
  if (( $# == 1 )); then
    if ! "${command[@]}" fmt --stdin --check < "$file"; then
      printf 'Unformatted Axonyx template: %s\n' "$file" >&2
      failed=1
    fi
  else
    temporary=$(mktemp "${file}.format-XXXXXX")
    if "${command[@]}" fmt --stdin < "$file" > "$temporary"; then
      if ! cmp -s -- "$file" "$temporary"; then
        cat -- "$temporary" > "$file"
      fi
    else
      printf 'Axonyx template formatting failed: %s\n' "$file" >&2
      failed=1
    fi
    rm -f -- "$temporary"
    temporary=''
  fi
  count=$((count + 1))
done < <(find crates/create-axonyx/templates crates/cargo-axonyx/templates -type f \
  \( -name '*.asx.tpl' -o -name '*.ax.tpl' \) -print0)
printf 'Processed %s Axonyx source templates.\n' "$count"
exit "$failed"
