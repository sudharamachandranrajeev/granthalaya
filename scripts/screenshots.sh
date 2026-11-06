#!/usr/bin/env bash
# Renders the README screenshots from synthetic demo data: the Rust core
# builds a demo paper, archive and book, and the UI runs in headless Chromium
# with the Tauri calls faked (app/harness/) and driven by scripts/shot.mjs. Nothing real is ever shown.
#
#   scripts/screenshots.sh     writes docs/screenshots/*.jpg
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
out=docs/screenshots
export CHROMIUM=${CHROMIUM:-chromium}
port=5198
mkdir -p "$out"

cargo run -q -p granthalaya-core --example demo >app/harness/demo.json
(cd app && exec npx vite --port $port --strictPort >/dev/null 2>&1) &
vite=$!
trap 'kill $vite 2>/dev/null || true' EXIT
for _ in $(seq 40); do curl -sf "http://localhost:$port/" >/dev/null && break; sleep 0.25; done

shot() { # name width,height hash
  node scripts/shot.mjs "http://localhost:$port/harness/index.html#$3" "$out/$1.jpg" "${2%,*}" "${2#*,}" "${4:-4000}"
  echo "$out/$1.jpg"
}
shot home 1280,800 "view=home"
shot paper 1280,1400 "view=paper"
shot story 1280,1300 "view=story"
shot archive 1280,900 "view=archive&q=flood%20barrier"
shot library 1280,900 "view=library"
shot catalogue 1280,1000 "view=browse"
shot reader 1280,800 "view=reader" 7000
shot notes 1280,800 "view=notes" 7000
shot phone-paper 390,844 "view=paper"
shot phone-reader 390,844 "view=reader&marks=0" 7000
