# Hosting GridCraft Viewer for the web

On the `office-viewer` branch the web app is GridCraft Viewer, which shows workbooks read-only
(`docs/viewer.md`).

`gridcraft-web-<version>.zip` (from the GitHub release, or `packaging/web/package.sh`) holds a
static site in `gridcraft-web-<version>/`:

| File | What it is |
|---|---|
| `index.html` | The page. It loads everything through relative URLs. |
| `gridcraft-web-<hash>.js` | wasm-bindgen glue (generated, ES module) |
| `gridcraft-web-<hash>_bg.wasm` | The app (compresses to roughly a third with gzip/Brotli) |
| `*.wasm.br`, `*.wasm.gz`, `*.js.br`, … | Precompressed copies (Brotli quality 11, gzip -9) for servers that send them as they are |
| `_headers`, `.htaccess` | Sample header rules for Netlify/Cloudflare Pages and Apache |

There is no server-side code. Upload the folder's contents anywhere that serves static files.

## Download size

`trunk build --release` builds with `[profile.web]` in the root `Cargo.toml` (`opt-level = "z"`,
fat LTO, one codegen unit, `strip`, `panic = "abort"`), runs `wasm-opt -Oz`, and builds
`gridcraft-ui-egui` without its `editor` feature and `gridcraft-engine` without its `macros`
feature. Measured on 2026-11-03 (wasm-opt 120, trunk
0.22.1):

| Build | wasm raw | wasm gzip -9 | wasm Brotli 11 | js raw / gzip / Brotli |
|---|---:|---:|---:|---:|
| `main` (release profile: opt-level 3, thin LTO, 16 codegen units) | 23.94 MB | 8.73 MB | 5.73 MB | 162 / 24 / 20 KB |
| + `[profile.web]` | 14.43 MB | 5.62 MB | 4.16 MB | 170 / 24 / 20 KB |
| + without the `editor` feature (no ribbon, title bar, keytips or formula bar) | 13.97 MB | 5.44 MB | 4.05 MB | 170 / 24 / 20 KB |
| + only the viewer's dialogs and File pages | 13.72 MB | 5.35 MB | 3.99 MB | 170 / 24 / 20 KB |
| + no macro editor, code editor or cell editor, and no macro runtime (this branch) | **13.42 MB** | **5.21 MB** | **3.89 MB** | 170 / 24 / 20 KB |

Brotli is what a browser downloads from a host that serves the `.br` copies: 3.89 MB in place of
5.73 MB (−32%). That is −44% raw and −40% gzip. No fonts are duplicated. The 1.4 MB of
embedded faces are egui's defaults and its monochrome emoji, which the UI needs for its symbols.

## Any path works

All URLs in `index.html` are relative (`public_url = "./"` in `apps/gridcraft-web/Trunk.toml`),
so the site works at a domain root (`https://example.com/`), under a prefix
(`https://example.com/tools/gridcraft/`) and from a CDN bucket. The asset names carry a content
hash, so they can be cached forever. Only `index.html` needs revalidation.

## Required server settings

- **MIME type:** serve `.wasm` as `application/wasm`. Browsers refuse to stream-compile it under
  any other type, and the app then loads slowly or not at all. Serve `.js` as `text/javascript`.
  Most hosts already do both. For nginx, check that `mime.types` has `application/wasm wasm;`.
- **Compression:** turn on gzip or Brotli for `.wasm`, `.js` and `.html`. That cuts the
  download to roughly a third. The package already holds precompressed `.br` and `.gz` copies.
  Serve them with nginx `gzip_static on;` (and `brotli_static on;` with the Brotli module), or use
  the `.htaccess` rewrite rules on Apache. Either way the server sends `Content-Encoding: br`
  or `gzip` and `Vary: Accept-Encoding`. Netlify and Cloudflare Pages compress on their own.
- **Caching:** `Cache-Control: public, max-age=31536000, immutable` on the hashed `.wasm` and
  `.js` files, and `no-cache` on `index.html`.
- **HTTPS:** WebGPU (and the clipboard) only work in a secure context, which means `https://`
  or `http://localhost`. Over plain HTTP elsewhere, the app falls back to WebGL2.
- **No special isolation headers:** GridCraft doesn't use `SharedArrayBuffer`, so it doesn't
  need `Cross-Origin-Opener-Policy` or `Cross-Origin-Embedder-Policy`. If your site already sends
  COEP `require-corp`, also send `Cross-Origin-Resource-Policy: same-origin` (or `cross-origin`
  when the files live on a CDN) on the app's files.

nginx example:

```nginx
location /gridcraft/ {
    types { application/wasm wasm; text/javascript js; text/html html; }
    gzip on;
    gzip_static on;   # sends the package's .gz copies (brotli_static on; with ngx_brotli for .br)
    gzip_types application/wasm text/javascript text/html;
    location ~* \.(wasm|js)$ { add_header Cache-Control "public, max-age=31536000, immutable"; }
    location ~* index\.html$ { add_header Cache-Control "no-cache"; }
}
```

Local test: `python3 -m http.server 8765` inside the folder, then open http://localhost:8765/.

## Embedding in a page (iframe)

```html
<iframe
  src="https://example.com/gridcraft/"
  title="GridCraft spreadsheet"
  style="width: 100%; height: 720px; border: 0;"
  allow="fullscreen; clipboard-read; clipboard-write"
  allowfullscreen>
</iframe>
```

- The app fills the iframe and follows its size, so size the iframe and not the app.
- Keyboard shortcuts go to the iframe after the user clicks into it, as with any embedded app.
- **Cross-origin embeds** work. Preferences are kept in the iframe's `localStorage`. Browsers
  that partition or block third-party storage may forget them between visits, and the app
  then starts with defaults.
- **Sandboxed iframes** need at least
  `sandbox="allow-scripts allow-same-origin allow-downloads allow-popups"`. Without
  `allow-same-origin` there's no storage. Without `allow-downloads`, Save and Export (browser
  downloads) are blocked.
- Don't send `X-Frame-Options: DENY` or a `frame-ancestors` CSP that excludes the embedding page.
- **Window chrome:** `?chrome=none` on the iframe `src` leaves out the title bar and the File tab
  (whose Backstage view then never opens); `?chrome=notitlebar` or `?chrome=nofiletab` leave out just
  one. Use them when the host window already shows the file name and owns the files, so the sheet's
  own Open or Save As cannot replace what the host thinks is open. Everything stays on by default.

## Renderer selection

GridCraft's web build is the same egui app as the desktop one, compiled to wasm
(`apps/gridcraft-web`). It renders with the backend `apps/gridcraft-web` selects (WebGPU
where available, otherwise WebGL2). If the web app adds URL flags to force a backend (as the
sibling Craft apps do with `?webgl` / `?cpu`), document them here; they also work on the
iframe `src`.

A browser with neither WebGPU nor WebGL2 gets a message in place of the app.
