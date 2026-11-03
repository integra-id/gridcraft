#!/usr/bin/env bash
# Build the browser version (GridCraft Viewer, `[profile.web]`) and zip it:  $DIST/gridcraft-web-<version>.zip
#
# Usage: packaging/web/package.sh [--skip-build]
#
# Needs: trunk (brew install trunk / cargo install trunk --locked) and the wasm32-unknown-unknown
# target. The zip holds a self-contained static site in gridcraft-web-<version>/ that works
# from any URL path and inside an <iframe>. Hosting notes: packaging/web/README.md.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/web"

SITE="$ROOT/dist/web"
if [[ "${1:-}" != "--skip-build" ]]; then
  command -v trunk >/dev/null || { echo "error: trunk not found (cargo install trunk --locked)" >&2; exit 1; }
  [[ -f "$ROOT/apps/gridcraft-web/index.html" ]] || { echo "error: apps/gridcraft-web/index.html missing (the web app isn't set up yet)" >&2; exit 1; }
  # --dist/--public-url here, so the output doesn't depend on Trunk.toml (which should agree:
  # public_url = "./").
  (cd "$ROOT/apps/gridcraft-web" && trunk build --release --dist "$SITE" --public-url ./)
fi

[[ -f "$SITE/index.html" ]] || { echo "error: $SITE/index.html missing; run without --skip-build" >&2; exit 1; }
# Paths must be relative so the site works under any prefix (public_url = "./" in Trunk.toml).
if grep -Eq '(src|href)="/[^/]' "$SITE/index.html"; then
  echo "error: $SITE/index.html has root-absolute URLs; it would break when served from a sub-path" >&2
  exit 1
fi

NAME="gridcraft-web-$VERSION"
WORK="$CARGO_TARGET_DIR/web-package"
rm -rf "$WORK"
mkdir -p "$WORK/$NAME"
cp -R "$SITE/." "$WORK/$NAME/"
# trunk leaves an empty lock file in the dist dir; it is not part of the site.
rm -f "$WORK/$NAME/.trunk-lock"
# Sample server configs (MIME type, caching, compression); harmless where unused.
cp "$HERE/_headers" "$HERE/.htaccess" "$WORK/$NAME/"
cp "$HERE/README.md" "$WORK/$NAME/HOSTING.md"
# Precompressed copies beside the wasm, js and html (`x.wasm.br`, `x.wasm.gz`) for hosts that
# serve them as they are (nginx `gzip_static`/`brotli_static`, the `.htaccess` rules): Brotli at
# quality 11 is far smaller than what a server compresses on the fly. `-n` keeps gzip output
# reproducible.
command -v brotli >/dev/null || echo "warning: brotli not found; packaging without .br files" >&2
while IFS= read -r -d '' f; do
  gzip -k9nf "$f"
  if command -v brotli >/dev/null; then brotli -kf -q 11 "$f"; fi
done < <(find "$WORK/$NAME" -maxdepth 1 -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.html' \) -print0)
copy_docs "$WORK/$NAME"
rm -f "$DIST/$NAME.zip"
(cd "$WORK" && zip -qr9 "$DIST/$NAME.zip" "$NAME")
echo "wrote $DIST/$NAME.zip"
ls -lh "$DIST/$NAME.zip"
