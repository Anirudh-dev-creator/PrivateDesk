# PrivateDesk

Offline Windows app. Built so far: app shell, **Target Size Compressor**, **Batch Compress**, **Image Studio**, **Passport & ID Photos**, **Utilities**. See `docs/STATUS.md`.
Default target: **100 KB maximum = 100,000 bytes**.

## Get the .exe from GitHub (no Rust or Node needed on your PC)

1. Create a new **empty** repository on github.com (e.g. `PrivateDesk`).
2. Upload **everything in this folder**, including the hidden-looking `.github` folder.
   Easiest: install **GitHub Desktop**, add this folder as a repository, commit, and push.
   (If you use the website's "Upload files", drag the whole folder contents and check that
   `.github/workflows/build-windows.yml` appears in the repository afterwards.)
3. Open the **Actions** tab on GitHub. The "Build PrivateDesk for Windows" workflow starts by itself
   (or click it, then **Run workflow**). The first build takes roughly 10–20 minutes.
4. When it shows a green tick, open the run and download from **Artifacts**:
   - `PrivateDesk-installer` – the setup `.exe`
   - `PrivateDesk-portable-exe` – `privatedesk.exe`, runs without installing
5. If the run shows a red cross, open it, click the failed step, and copy the error text.
   The build was **not compiled by the author before delivery**, so first-run errors are possible.

The workflow runs the compressor's automated tests first. If any test fails, no installer is produced.

### Windows notes
- The exe is **unsigned**, so Windows SmartScreen may warn: click *More info* → *Run anyway*.
- The app needs **WebView2** (already part of Windows 11 and updated Windows 10). The installer
  downloads it if missing, which needs internet once.
- **PDF compression needs Ghostscript** (free): install it from ghostscript.com. PrivateDesk finds it
  automatically. Ghostscript is AGPL-licensed, so it is *not* bundled. Check its license before
  redistributing it with your own installer. Image compression needs nothing extra.

## Run on your own PC (optional)
Install Node 20+, Rust (rustup) and the Visual Studio C++ Build Tools, then:
```
npm install
npm run tauri dev
```
Run the engine tests: `cd crates/compressor && cargo test --release` and `cd crates/tools && cargo test --release`

## Layout
- `crates/compressor` – compression engine (plain Rust, unit-tested, no network code)
- `crates/tools` – image editing, passport photos and file utilities (plain Rust, unit-tested)
- `src-tauri` – Tauri 2 desktop shell and commands
- `src` – React + TypeScript interface
- `.github/workflows/build-windows.yml` – cloud build
- `docs/STATUS.md` – what works, what does not

## Privacy
No network code for your files. The window's content-security policy blocks web requests.
Original files are never overwritten; outputs are saved only to a new file name you choose.
