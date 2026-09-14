<h1 align="center">TransFEL</h1>

<p align="center">
  <strong>Mirror, control and transfer files between your Android device and your PC.</strong>
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%2B-0078D6">
  <img alt="Desktop" src="https://img.shields.io/badge/desktop-Rust%20%2B%20egui-CE422B">
  <img alt="Mobile" src="https://img.shields.io/badge/android-Kotlin%20%2B%20Compose-3DDC84">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-blue">
</p>

---

TransFEL streams your Android screen to your computer in real time and lets you browse
and transfer files in both directions. It ships as a single installer: no Rust, no
Android Studio, no ADB and no FFmpeg to set up by hand.

The project has two parts — a **Kotlin** app that runs on the phone and a **Rust**
desktop application for Windows.

## Screenshots

> _Add screenshots here: `docs/screenshots/mirror.png`, `docs/screenshots/files.png`_

## Features

**Screen mirroring**
- Real-time capture through `MediaProjection`, encoded on-device with `MediaCodec` (H.264).
- Hardware-accelerated decoding on the desktop via FFmpeg.
- Automatic scaling to the window, with the device's native aspect ratio preserved.
- Live connection status, with a clear "connection closed" state instead of a frozen frame.

**Device handling**
- Automatic detection when a device is plugged in or unplugged — no refresh button needed.
- Model, Android version, resolution and screen density reported at a glance.
- ADB reverse tunnel configured automatically on connect.

**File manager**
- Side-by-side browsing of the computer and the device.
- Filter by type: photos, documents, video, audio.
- Quick access to Camera, Gallery, Downloads, Videos, Music and WhatsApp media.
- Search inside the current folder, or across the entire device.
- Multi-file staging tray: pick files from either side, then transfer them in one action.
- Background transfers with per-file status; the interface never blocks.

## How it works

```text
┌──────────────────────────────┐
│        Android device        │
│                              │
│   MediaProjection            │
│        │                     │
│        ▼                     │
│   MediaCodec  ──►  H.264     │
│        │                     │
│        ▼                     │
│   TCP socket (port 5000)     │
└────────────┬─────────────────┘
             │  ADB reverse / Wi-Fi
             ▼
┌──────────────────────────────┐
│       TransFEL Desktop       │
│                              │
│   TCP server                 │
│        │                     │
│        ▼                     │
│   FFmpeg  ──►  RGBA frames   │
│        │                     │
│        ▼                     │
│   egui renderer              │
└──────────────────────────────┘
```

The desktop application listens on `127.0.0.1:5000`. `adb reverse` maps that port into
the device, so the phone connects to what it sees as its own localhost — this keeps the
transport identical whether the link is USB or Wi-Fi.

File operations go through `adb push` / `adb pull`. Media listings are read from
Android's `MediaStore` index rather than by walking the filesystem, which keeps browsing
instant even on devices with tens of thousands of photos.

## Requirements

| | |
|---|---|
| Desktop | Windows 10 or later, 64-bit |
| Device | Android 7.0 (API 24) or later |
| Connection | USB cable with data support (Wi-Fi support is in development) |

## Installation

### 1. Install the desktop application

Download `TransFEL-Setup.exe` from the
[Releases](https://github.com/pipeeex/TransFEL/releases) page and run it.

The installer bundles ADB and FFmpeg locally inside the application folder. Nothing is
added to the system `PATH` and no existing ADB or FFmpeg installation is modified.

### 2. Install the Android application

Download `TransFEL.apk` from the same Releases page and install it on your device. You
will need to allow installation from unknown sources — Android will prompt you.

### 3. Enable USB debugging

On your Android device:

1. Open **Settings → About phone**.
2. Tap **Build number** seven times to unlock Developer options.
3. Open **Settings → System → Developer options**.
4. Enable **USB debugging**.
5. Connect the device to the computer with a USB cable.
6. Accept the authorization prompt that appears on the phone.

### 4. Connect

1. Launch TransFEL on your computer. The device is detected automatically.
2. Open the TransFEL app on your phone and start the broadcast.
3. Grant the screen capture permission when Android asks.

The screen appears in the desktop window within a second or two.

## Usage

### Mirroring

Select **Screen** in the sidebar. The stream starts as soon as the phone begins
broadcasting and stops cleanly when it ends. The sidebar always reflects the current
state: *active*, *closed* or *idle*.

### Transferring files

1. Select **Files** in the sidebar.
2. Choose **PC** or **Phone** at the top.
3. Navigate, use a quick-access shortcut, or search.
4. Click **Add** on each file you want — they collect in the tray on the right.
5. Set the destination folder and click **Transfer all**.

Files staged from the PC are sent to the phone; files staged from the phone are pulled to
the current PC folder. You can mix both directions in a single batch.

## Project layout

```text
TransFEL/
├── android/                 Kotlin application (Jetpack Compose)
├── desktop/                 Rust application
│   ├── src/
│   │   ├── main.rs          Entry point and window setup
│   │   ├── app.rs           Application state and frame loop
│   │   ├── adb.rs           ADB and FFmpeg process wrappers
│   │   ├── device.rs        Device detection and properties
│   │   ├── watcher.rs       Connect / disconnect event stream
│   │   ├── stream.rs        TCP server and H.264 decoding
│   │   ├── files.rs         File manager model and workers
│   │   ├── theme.rs         Color tokens
│   │   └── ui/              Interface modules
│   └── Cargo.toml
├── tools/                   Bundled adb.exe and ffmpeg.exe
└── installer/               Inno Setup script
```

Installed layout:

```text
C:\Program Files\TransFEL\
├── TransFEL.exe
├── tools\
│   ├── adb.exe
│   ├── AdbWinApi.dll
│   ├── AdbWinUsbApi.dll
│   └── ffmpeg.exe
└── resources\
```

## Building from source

### Desktop

Requires the [Rust toolchain](https://rustup.rs/) (stable).

```bash
git clone https://github.com/YOUR_USERNAME/TransFEL.git
cd TransFEL/desktop
cargo build --release
```

The binary lands in `target/release/TransFEL.exe`. Place `adb.exe` and `ffmpeg.exe` in a
`tools/` folder next to it — the application resolves them relative to its own path and
falls back to the system `PATH` if they are absent.

### Android

Open the `android/` folder in Android Studio (Giraffe or later) and run:

```bash
./gradlew assembleRelease
```

## Troubleshooting

**The device is not detected**
Confirm USB debugging is enabled and that you accepted the authorization prompt on the
phone. Try a different cable — charge-only cables carry no data. Then run
`tools\adb.exe devices` from the installation folder and check that your device is listed
as `device` and not `unauthorized`.

**The screen stays black after starting the broadcast**
The screen capture permission was likely denied. Close the phone app, reopen it and
accept the prompt.

**Some folders on the phone appear empty**
Android 11 and later restrict access to `Android/data` and `Android/obb`. This is a
platform limitation and affects every file manager, not just TransFEL.

**Transfers fail with a permission error**
Choose a destination inside `/sdcard`. System paths are read-only over ADB without root.

## Roadmap

- [ ] Wi-Fi connection, with no cable required after the first pairing
- [ ] Remote control: keyboard and touch input from the desktop
- [ ] Transfer progress per file, with speed and ETA
- [ ] Drag and drop onto the application window
- [ ] Configurable bitrate and resolution
- [ ] Linux and macOS builds

## Uninstalling

Remove TransFEL from **Settings → Apps** or through the bundled uninstaller. It removes
the application, the bundled ADB and FFmpeg binaries, application resources, and the
configuration and cache files TransFEL created.

Files you transferred to your computer are never touched, and no system-wide software is
modified or removed.

## Privacy

TransFEL runs entirely on your local network. No video, file or device information is
sent to any external server, and the application makes no network requests beyond the
direct connection between your phone and your computer.

## License

Released under the MIT License. See [LICENSE](LICENSE) for details.

ADB is part of the Android SDK Platform-Tools, distributed by Google under the Android
Software Development Kit License Agreement. FFmpeg is distributed under the LGPL 2.1 or
later.