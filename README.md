# Arbuz Launcher

A Minecraft launcher for Windows, macOS and Linux. Tauri + React on the inside,
no telemetry, no accounts required for co-op.

## Download

Releases: https://github.com/arbuzlauncher/arbuz-launcher/releases

- **Windows** — `Arbuz-Launcher-<version>-windows-x64-setup.exe` (installer) or
  `...-windows-x64-portable.exe` (single file). The installer puts the launcher in
  `%USERPROFILE%\arbuz-launcher`.
- **macOS** — `...-macos-universal.dmg`, works on both Apple Silicon and Intel.
- **Linux** — `...-linux-x86_64.AppImage`, `...-linux-amd64.deb` or `...-linux-x86_64.rpm`.

Every release also ships `SHA256SUMS.txt` if you want to check the downloads.

The binaries are not signed. Windows SmartScreen will ask for confirmation
("More info" → "Run anyway"), and on macOS you may need to right-click the app
and pick "Open" the first time.

Launcher data (settings, accounts, profiles, Java, worlds) is kept next to the
launcher, not in AppData: in a folder called `arbuz-launcher` on Windows, or in
`~/arbuz-launcher` on macOS and Linux. Set `ARBUZ_DATA_DIR` if you want another
location. Backups are just a copy of that folder.

## Build from source

You need Node.js 22+, pnpm 12+ and Rust stable. Tauri also needs the platform
toolchain:

**Windows** — Visual Studio Build Tools with the C++ workload, WebView2
(already present on Windows 10/11) and NSIS for the installer
(`choco install nsis`, or let Tauri download it on the first build).

**macOS**

```bash
xcode-select --install
```

**Linux (Ubuntu/Debian)**

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libdbus-1-dev libssl-dev patchelf build-essential
```

Then:

```bash
pnpm install --frozen-lockfile
pnpm tauri dev   # run with hot reload
pnpm check       # TypeScript
pnpm test        # Rust tests
```

Builds:

```bash
# Windows: exe + NSIS installer
pnpm run dist

# macOS: universal dmg
pnpm tauri build --target universal-apple-darwin --bundles dmg

# Linux
pnpm tauri build --bundles deb,rpm,appimage
```

The output is in `src-tauri/target/<target>/release/bundle/`. Anything under
`dist/` or `src-tauri/target/` is ignored by git.

The version is stored in `package.json`, `tauri.conf.json` and `Cargo.toml`.
The launcher uses the Cargo value to check for updates, so bump all three at
once:

```bash
pnpm version:set 1.2.3
```

## License

GPL-3.0. See [LICENSE](LICENSE).
