#!/usr/bin/env bash
# Preview user docs, optionally bundled with rustdoc.
set -euo pipefail

if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  echo 'Usage: scripts/serve-docs.sh [--watch]'
  exit 0
fi
PORT="${PORT:-$(python3 - <<'PY'
import socket

for port in range(9000, 9100):
    with socket.socket() as sock:
        try:
            sock.bind(("127.0.0.1", port))
        except OSError:
            continue
        print(port)
        break
else:
    raise SystemExit("no free preview port in 9000–9099")
PY
)}"
if [[ "${1:-}" == --watch ]]; then
  uvx zensical build --strict --clean
  exec uvx zensical serve --dev-addr "localhost:$PORT"
fi
uvx zensical build --strict --clean
cargo doc --workspace --no-deps --locked
mkdir -p site/api
cp -r target/doc/. site/api/
exec python3 -m http.server "$PORT" --directory site --bind 127.0.0.1
