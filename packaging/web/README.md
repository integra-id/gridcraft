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
`gridcraft-ui-egui` without its `editor` feature and `gridcraft-engine` without its `editing` and
`macros` features. Measured on 2026-11-03 (wasm-opt 120, trunk
0.22.1):

| Build | wasm raw | wasm gzip -9 | wasm Brotli 11 | js raw / gzip / Brotli |
|---|---:|---:|---:|---:|
| `main` (release profile: opt-level 3, thin LTO, 16 codegen units) | 23.94 MB | 8.73 MB | 5.73 MB | 162 / 24 / 20 KB |
| + `[profile.web]` | 14.43 MB | 5.62 MB | 4.16 MB | 170 / 24 / 20 KB |
| + without the `editor` feature (no ribbon, title bar, keytips or formula bar) | 13.97 MB | 5.44 MB | 4.05 MB | 170 / 24 / 20 KB |
| + only the viewer's dialogs and File pages | 13.72 MB | 5.35 MB | 3.99 MB | 170 / 24 / 20 KB |
| + no macro editor, code editor or cell editor, and no macro runtime | 13.42 MB | 5.21 MB | 3.89 MB | 170 / 24 / 20 KB |
| + no editing commands in the engine (this branch) | **12.25 MB** | **4.73 MB** | **3.59 MB** | 170 / 24 / 20 KB |

Brotli is what a browser downloads from a host that serves the `.br` copies: 3.59 MB in place of
5.73 MB (−37%). That is −49% raw and −46% gzip. No fonts are duplicated. The 1.4 MB of
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

## Opening a workbook from an address

`?url=` (or its alias `?src=`; `?url=` wins if both are given) opens a workbook when the page
starts, read-only, as if it had been picked in File › Open:

```
https://example.com/gridcraft/?url=/files/budget.xlsx
https://example.com/gridcraft/?url=https%3A%2F%2Ffiles.example.org%2Fq3.ods&name=Q3.ods
<iframe src="https://example.com/gridcraft/?src=../reports/sales.xlsx" ...>
```

- **Addresses.** Relative addresses are resolved against the viewer page; absolute ones are
  fetched as they are. Only `http:`, `https:` and `blob:` are fetched; `javascript:`, `data:`,
  `file:` and the rest are refused. Percent-encode an address that has its own `?` or `&`
  (`encodeURIComponent`), or its query becomes the viewer's.
- **Formats.** Whatever File › Open reads: `.xlsx`, `.xlsm`, `.xls`, `.xlsb`, `.ods`, `.csv`,
  `.tsv`, `.txt`, GridCraft `.json`. The engine tells xlsx, xls, xlsb and ods apart by their
  content, so an address without an extension (`/download?id=5`) works for those; a text file
  needs a `.csv`/`.tsv` name or a `text/csv` type.
- **Name.** `?name=` if given, else the `Content-Disposition` file name, else the address's last
  path segment (decoded, without its query), else `workbook.xlsx`. Only a file name is kept. A
  name without a spreadsheet extension gets one from `Content-Type` when it is a spreadsheet type.
- **Cookies** go only to the viewer's own origin (`credentials: same-origin`); a file behind
  another site's login can't be opened this way.
- **CORS.** A file on another origin can be read only if its server sends
  `Access-Control-Allow-Origin` with the viewer's origin (or `*`). The viewer makes a plain `GET`,
  so no preflight is needed. To use that server's `Content-Disposition` name, also send
  `Access-Control-Expose-Headers: Content-Disposition`. For example, in nginx on the file server:

  ```nginx
  location /files/ {
      add_header Access-Control-Allow-Origin "https://example.com" always;
      add_header Access-Control-Expose-Headers "Content-Disposition, Content-Length" always;
  }
  ```

  or in Apache (`.htaccess` next to the files, `mod_headers`):

  ```apache
  Header always set Access-Control-Allow-Origin "https://example.com"
  Header always set Access-Control-Expose-Headers "Content-Disposition, Content-Length"
  ```

  Without it the browser blocks the download and the viewer says so. Files on the viewer's own
  server need none of this.
- **While it downloads** the sheet shows *Opening <name>…*. A bad address, a network or CORS
  failure, an HTTP error (404, 403, …), an empty file, one larger than 512 MB or one GridCraft
  can't read shows a message box (*Couldn't open name: why*) and is logged to the console; the
  viewer stays usable.
- `?url=` takes precedence over `?sample`, and is ignored with `?host=parent` (the parent page
  sends the workbook then).

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

## Query parameters

| Parameter | Effect |
|---|---|
| `?webgl` | Force the WebGL2 backend |
| `?sample=sales\|budget\|grades` | Open a built-in sample |
| `?url=ADDRESS` / `?src=ADDRESS` (`&name=NAME`) | Open the workbook at that address (*Opening a workbook from an address*) |
| `?chrome=none\|notitlebar\|nofiletab` | Leave out the title bar and/or the File tab |
| `?host=parent` (`&author=`) | Same-origin host page sends the workbook and stores saves |
