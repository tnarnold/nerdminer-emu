# NerdMiner Emulator (Rust)

**English** | [Português (BR)](README.pt-BR.md)

A desktop emulator of [NerdMiner v2](https://github.com/BitMaker-hub/NerdMiner_v2): a **solo**
Bitcoin "lottery" miner with the same T-Display (320x170) interface. It runs as a small "dock" on
your desktop, can hide in the system tray, and lets you control how much CPU and GPU it uses.

Runs on **macOS, Linux and Windows**.

🌐 **[Website](https://tnarnold.github.io/nerdminer-emu/)** · 📖 **[User manual](docs/MANUAL.md)**: setup, every screen and setting, how it works, troubleshooting.

## Features

- The 4 original screens (Miner, Clock, Global Stats and BTC Price), using the bitmaps, fonts and
  coordinates from the original firmware. Click the screen to switch (like the device button);
  drag to move the window.
- Stratum v1 (public-pool.io, solo.ckpool.org or any solo pool), with `mining.suggest_difficulty`.
- **Mining device selection: CPU, GPU or CPU + GPU**, each with its own usage limit, adjustable
  live without restarting:
  - **CPU**: number of threads + % usage per thread (duty cycle). Uses the processor's SHA
    instructions when available (ARMv8 / SHA-NI).
  - **GPU**: % usage (duty cycle) of a WGSL compute shader (wgpu → Metal / Vulkan / DX12).
    The GPU is **off** by default and only used when selected.
- System tray as a mini dashboard: status dot, mining stats, **network occupancy and the fee to use right now** (with the cost in sats and your currency), quick CPU/GPU controls. On macOS the fields you choose appear in the menu bar.
- Borderless window, "always on top" option, 1x–2.5x zoom, start hidden.
- Discreet Go BTC logo in the top-left corner; clicking it opens https://gobtc.com.br.
- BTC price in your currency (BRL, USD, EUR, GBP, CAD, CHF, AUD, JPY).
- Network data (price, block height, global hashrate, fees, halving) from mempool.space.

> Solo mining with a CPU/GPU is a lottery: the chance of finding a block is tiny.
> The goal is fun and learning, just like the physical NerdMiner.

## Resource usage

Measured on a MacBook Air M1 (8 cores), `--release` build, mining against a local pool.
Process CPU is shown the way `top` reports it (100% = one full core; 800% = the whole machine).

| Selection | Process CPU | % of total CPU | Memory (phys_footprint) |
|---|---:|---:|---:|
| CPU, 4 threads @ 50% (default) | ~180% | ~22% | ~40 MB |
| CPU, 4 threads @ 100% | ~393% | ~49% | ~41 MB |
| CPU, 8 threads @ 25% | ~150% | ~19% | ~40 MB |
| CPU, 1 thread @ 10% | ~9% | ~1% | ~40 MB |
| GPU @ 30% | ~2% | <1% | ~41 MB |
| GPU @ 100% | ~5% | <1% | ~42 MB |
| CPU 4 @ 50% + GPU @ 30% | ~179% | ~22% | ~43 MB |

- **Memory**: stays between ~28 and ~45 MB and is stable over time (no growth). The UI (egui)
  and the 5 screen textures account for most of it; the miner itself uses a few KB per thread.
  With the GPU enabled, add ~10 MB of buffers/driver.
- **CPU**: usage follows `threads × % per thread`. With only the GPU selected, the CPU stays
  almost idle (it just coordinates and verifies candidates).
- **GPU**: the % controls how long the GPU is kept busy (each dispatch takes ~30 ms, followed by a
  proportional pause), so the desktop stays responsive.
- Reference hashrate on the M1: ~6.5 MH/s per CPU thread at 100% (4 threads ≈ 26 MH/s) and
  ~53 MH/s on the GPU at 100%.

## Install

Download the installer for your platform from the
[Releases](https://github.com/tnarnold/nerdminer-emu/releases) page:

- **macOS** (Apple Silicon and Intel): `NerdMiner-Emulator-<version>-macos-universal.dmg`. Open the
  `.dmg` and drag **NerdMiner Emulator** into **Applications**. The app is not notarized by Apple,
  so the first time right-click the app → **Open** → **Open** (or run
  `xattr -dr com.apple.quarantine "/Applications/NerdMiner Emulator.app"`).
- **Windows**: `...-windows-x86_64.zip`, extract it and run `nerdminer-emu.exe`.
- **Linux**: `...-linux-x86_64.tar.gz`, extract it and run `./nerdminer-emu`.

### Build the .dmg locally

```sh
scripts/package-macos.sh   # -> dist/NerdMiner-Emulator-<version>-macos-universal.dmg
```

With an Apple Developer account you can sign and notarize it by setting `MACOS_SIGN_IDENTITY`
and `MACOS_NOTARY_PROFILE` (see the top of the script).

### Publish a release

```sh
git tag v0.1.0 && git push origin v0.1.0
```

The `release` workflow builds the `.dmg`, `.zip` and `.tar.gz` and creates the GitHub Release.

## Build and run

Requires stable Rust (https://rustup.rs).

```sh
cargo run --release
```

### Linux dependencies

```sh
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev \
  libxi-dev libgl1-mesa-dev libvulkan-dev libdbus-1-dev pkg-config
```

The Linux tray uses StatusNotifierItem (D-Bus, `ksni` crate). GNOME needs the
"AppIndicator and KStatusNotifierItem Support" extension; KDE, XFCE, Cinnamon etc. support it
out of the box.

## Configuration

See the [user manual](docs/MANUAL.md#5-settings-) for every option. In short: on first launch the ⚙ panel opens: enter your **BTC address** and click
*Salvar e reconectar* (save and reconnect). The configuration is stored at:

- macOS: `~/Library/Application Support/nerdminer-emu/config.json`
- Linux: `~/.config/nerdminer-emu/config.json`
- Windows: `%APPDATA%\nerdminer-emu\config.json`

Useful environment variables for development:

- `NERDMINER_CONFIG=/path/config.json`: use a different configuration file.
- `NERDMINER_SHOTS=/directory`: save raw RGBA screenshots of every screen, then exit.
- `NERDMINER_TRAY_DUMP=1`: print the tray menu (title, tooltip, every line) to stderr on each refresh.

### The pool doesn't connect

The miner only makes **outbound** connections to the pool (like a web browser), so it **does not
need UPnP or port forwarding**. If the dot next to the wifi icon on the Miner screen stays red,
your network is probably blocking the outbound port. Check with:

```sh
nc -vz public-pool.io 21496     # solo.ckpool.org uses port 3333
```

If it times out, allow outbound TCP on that port in your firewall, or try another network (e.g. a
phone hotspot). The detailed reason is shown in the log inside the ⚙ panel.

## Tests

```sh
cargo test --release -- --nocapture
```

Includes the genesis block hash (midstate path) and a GPU test that must find the real genesis
block nonce.

## Project layout

| File | Purpose |
|---|---|
| `src/work.rs` | block header, coinbase/merkle, SHA-256d with midstate, difficulty |
| `src/stratum.rs` | Stratum v1 client |
| `src/miner.rs` | CPU threads with usage limit, hashrate calculation |
| `src/gpu.rs`, `src/sha256d.wgsl` | GPU miner (wgpu) |
| `src/screens.rs` | NerdMiner screens (bitmaps, fonts, coordinates) |
| `src/app.rs` | window, toolbar and settings |
| `src/tray.rs` | system tray |
| `src/netinfo.rs` | network data from mempool.space |

## Credits

UI based on NerdMiner v2 (© 2023 Bitmaker, MIT). See `assets/CREDITS.md`.
