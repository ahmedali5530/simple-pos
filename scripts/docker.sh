#!/bin/sh
# Build the till image and serve it on 127.0.0.1:8787 for Caddy or Nginx.
set -eu
cd "$(dirname "$0")/.."

if [ ! -f .env ]; then
  echo "Copy .env.example to .env and set DATABASE_URL." >&2
  exit 1
fi

db=$(grep -E '^[[:space:]]*DATABASE_URL=' .env | tail -1 || true)
case "$db" in
  "")
    echo "Set DATABASE_URL in .env." >&2
    exit 1
    ;;
esac

if docker compose version >/dev/null 2>&1; then
  compose="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
  compose="docker-compose"
else
  echo "Docker is installed, but Docker Compose is not." >&2
  echo "Install it, then run this script again:" >&2
  echo "  sudo apt-get install docker-compose-plugin" >&2
  exit 1
fi

# $compose is either "docker compose" or "docker-compose".
# shellcheck disable=SC2086
$compose up --detach --build "$@"
echo "Till is on http://127.0.0.1:8787"
echo "Point the domain at that address."
