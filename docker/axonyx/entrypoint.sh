#!/bin/sh
set -eu

PORT="${PORT:-${AXONYX_PORT:-3000}}"
HOST="${AXONYX_HOST:-0.0.0.0}"

cat <<EOF
Axonyx demo is running.

Open: http://localhost:${PORT}
Template: docs
CLI: cargo-axonyx ${CARGO_AXONYX_VERSION:-0.1.87}
Runtime: axonyx-runtime ${AXONYX_RUNTIME_VERSION:-0.1.48}

EOF

exec cargo ax run start --host "${HOST}" --port "${PORT}"
