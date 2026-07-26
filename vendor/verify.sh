#!/usr/bin/env bash
# Verifies vendored assets still match the upstream releases they claim to be,
# and that no page has quietly reintroduced a third-party CDN reference.
#
# Run from anywhere:  ./vendor/verify.sh
set -uo pipefail

cd "$(dirname "$0")/.."
status=0

check() {
  local file="$1" url="$2" expected="$3"
  printf '  %-28s ' "$(basename "$file")"

  local actual
  actual="sha384-$(openssl dgst -sha384 -binary "$file" | openssl base64 -A)"
  if [[ "$actual" != "$expected" ]]; then
    echo "FAIL: local file does not match the recorded hash"
    echo "      recorded: $expected"
    echo "      actual:   $actual"
    status=1
    return
  fi

  # Upstream check is advisory: no network is a skip, not a failure, so this
  # stays usable offline and in CI sandboxes.
  local upstream
  if ! upstream="$(curl -fsL --max-time 20 "$url" 2>/dev/null | openssl dgst -sha384 -binary | openssl base64 -A)"; then
    echo "OK (hash matches; upstream unreachable, skipped)"
    return
  fi

  if [[ "sha384-$upstream" == "$expected" ]]; then
    echo "OK (matches upstream)"
  else
    echo "WARN: upstream has changed at this URL"
    echo "      This should not happen for a pinned version. Investigate before updating."
    status=1
  fi
}

echo "Verifying vendored assets:"
check vendor/pico.classless.min.css \
  "https://cdn.jsdelivr.net/npm/@picocss/pico@2.1.1/css/pico.classless.min.css" \
  "sha384-NZhm4G1I7BpEGdjDKnzEfy3d78xvy7ECKUwwnKTYi036z42IyF056PbHfpQLIYgL"

echo
echo "Checking for reintroduced third-party references:"
# Exclude vendor/ itself, which legitimately documents the upstream URL.
if hits=$(grep -rn --include='*.html' --include='*.css' \
            -E 'https?://(cdn\.|unpkg|.*\.googleapis|.*\.cloudflare)' . 2>/dev/null \
          | grep -v '^\./vendor/'); then
  echo "$hits" | sed 's/^/  /'
  echo "  FAIL: pages should reference vendored copies, not a CDN."
  status=1
else
  echo "  OK: no third-party CDN references in any page."
fi

exit $status
