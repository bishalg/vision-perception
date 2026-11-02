# Curv Console desktop shell (`curv_desktop`)

A Tauri 2 window that opens the clinic server's Curv Console. WP-D.9, ADR-0011
decision 10. It contains no console code and no product logic: the console is the UI,
served by the clinic server.

## Why remote mode

The console cannot be bundled into the app. Since WP-C.16 every console route is
dynamic (per-request CSP nonce, `connection()`), so there is no static export, and
bundling it would mean shipping a Node runtime as a sidecar (rejected in ADR-0011,
Alternatives). The console is useless without the server anyway. So the shell loads the
server's URL directly, and every server update reaches the desktop with no app release.

Use it only when a binary is genuinely needed (kiosk, OS integration, a clinic that bans
browsers). Otherwise a browser or the installed PWA is the desktop client.

## Use

1. First start: a small local setup page asks for the clinic server address. It must be
   `https://`. Plain `http://` is accepted only for `localhost` / `127.0.0.1` (a console
   on the same machine, development).
2. The address is stored in the OS app config directory as `server.json`:
   - macOS: `~/Library/Application Support/com.open-spectrum.curv.desktop/`
   - Windows: `%APPDATA%\com.open-spectrum.curv.desktop\`
   - Linux: `~/.config/com.open-spectrum.curv.desktop/`
3. To change it later, use the menu **Curv Console > Change Clinic Server…**. A stored
   value that no longer passes validation is ignored and the setup page is shown again.

## Build

Prerequisites: Rust stable (`rust-toolchain.toml`), the Tauri CLI
(`cargo install tauri-cli --version "^2" --locked`), and on Linux the WebKitGTK 4.1
stack:

```bash
sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev patchelf build-essential file
```

Windows needs WebView2 (preinstalled on Windows 10/11) and the MSVC build tools.

```bash
cd apps/curv_desktop
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                          # URL validation + navigation allow-list
cargo deny check                    # licences/advisories (deny.toml)
cargo tauri dev                     # debug run (devtools available)
cargo tauri build --bundles app     # macOS .app (unsigned); dmg | msi,nsis | deb,rpm,appimage
```

Output goes to `apps/curv_desktop/target/` (gitignored, about 3 GB with debug and
release builds). Bundles land in `target/release/bundle/`.

This is its own Cargo project, outside the `rust/` workspace (empty `[workspace]` in
`Cargo.toml`), so it never touches `rust/Cargo.lock`.

CI: `.github/workflows/desktop.yml` runs fmt, clippy, tests and `cargo deny`, then
`cargo tauri build` on macOS, Windows and Linux, unsigned, and uploads the bundles as
workflow artifacts. Signing and notarisation are not set up; macOS will show the
unidentified-developer warning and Windows SmartScreen will warn until they are.

## Security model

The remote console is treated as untrusted web content in a hardened browser window.

- **Two separate windows.** `setup` shows the bundled static page (`setup/`); `main`
  shows the remote console. They are never the same webview.
- **No IPC for remote content.** The only app command is `set_server_url`. It is declared
  in the app ACL manifest (`build.rs`), so a webview can call it only with the
  `allow-set-server-url` permission. The single capability
  (`capabilities/setup.json`) grants just that permission, to the `setup` window, for
  local content only. No capability has a `remote` section, so the console window has no
  Tauri API at all, including no opener, fs or shell access. The command also checks the
  caller's window label and that its URL is the bundled app origin, and re-validates the
  URL in Rust.
- **Navigation pinned to one origin.** The console window loads only URLs with the
  configured server's scheme, host and port (plus `blob:` URLs created by that origin,
  for downloads, and inert `about:blank`). Other `http(s)` and `mailto` links open in the
  system browser through the opener plugin (called from Rust, not from the page); every
  other scheme (`file:`, `data:`, `javascript:`, custom) is blocked. `window.open` and
  `target=_blank` never create a second webview. The rules are pure functions in
  `src/policy.rs` with unit tests.
- **Setup window navigation** is limited to the bundled app origin.
- **Downloads** are allowed only from the server origin, to the OS default download
  folder.
- **Transport.** `https` only, except loopback. No credentials in the URL. TLS is
  validated by the OS webview; there is no certificate bypass.
- **CSP for the local page.** `tauri.conf.json` sets a strict CSP for the bundled setup
  page (`default-src 'none'`, scripts and styles from self only, IPC only). The console's
  own CSP comes from the server (per-request nonce) and is not altered by the shell.
- **Devtools** exist only in debug builds (`tauri` is built without the `devtools`
  feature, and windows set `devtools(cfg!(debug_assertions))`).
- **No product logic here.** Per the project rules, scoring and all other core logic stay
  in the Rust core and the server; this crate only frames the console.

## Licences

All crates are MIT/Apache-2.0 or other green-list licences, except five MPL-2.0 crates
pulled in unmodified by Tauri (`cssparser`, `cssparser-macros`, `dtoa-short`,
`selectors` via `tauri-utils`; `option-ext` via `dirs`). MPL-2.0 is yellow-listed in
`docs/license_policy.md` and allowed for unmodified sources, the same treatment as UniFFI
in `rust/deny.toml`; they are listed as exceptions in `deny.toml`. On Linux the app links
the system WebKitGTK/GTK libraries (LGPL) dynamically, as the policy requires.
