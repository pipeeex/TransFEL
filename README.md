<h1 align="center">TransFEL</h1>

<p align="center">
  <strong>Mirror your Android screen, control it with your mouse, and move files
  between phone and PC — over USB or Wi-Fi.</strong>
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-0078D6">
  <img alt="Desktop" src="https://img.shields.io/badge/desktop-Rust%20%2B%20egui-CE422B">
  <img alt="Mobile" src="https://img.shields.io/badge/android-Kotlin%20%2B%20Compose-3DDC84">
  <img alt="License" src="https://img.shields.io/badge/license-GPL--3.0-blue">
</p>

---

TransFEL streams your Android screen to your computer in real time, lets you drive
the phone with your mouse, and transfers files in both directions. After a one-time
setup over USB, everything works wirelessly on your local network.

The project has two parts — a **Kotlin** app that runs on the phone and a **Rust**
desktop application.

## Screenshots

> _Add screenshots here: `docs/screenshots/mirror.png`, `docs/screenshots/files.png`,
> `docs/screenshots/connection.png`_

## Features

**Screen mirroring**

- Real-time capture through `MediaProjection`, encoded on-device with `MediaCodec` (H.264).
- Decoding on the desktop via FFmpeg, scaled to the window with the native aspect ratio kept.
- Rotation is instant: the capture surface is square, so turning the phone never
  restarts the encoder, the decoder or the connection.
- Pause and resume from the phone, or from the notification, without tearing down
  the connection.
- Explicit connection states — live, paused, closed — instead of a frozen last frame.

**Remote control**

- Tap, long press, real-time drag and mouse-wheel scrolling, straight on the mirrored screen.
- Back, Home and Recents buttons.
- Runs through an `AccessibilityService`, so it needs no root and works over Wi-Fi
  without ADB. Enable it once in Android's accessibility settings.

**Connectivity**

- Automatic detection when a device is plugged in or unplugged.
- One-click Wi-Fi setup: TransFEL opens the port, reads the device IP and connects for you.
- Wireless pairing with a code for Android 11 and later, no cable at all.
- Remembers the last address and reconnects on launch.
- Multiple devices are listed and switchable; Wi-Fi is preferred so unplugging changes nothing.

**File manager**

- Side-by-side browsing of the computer and the device.
- Quick access to Camera, Gallery, Downloads, Videos, Music and WhatsApp media.
- Filter by type — photos, documents, video, audio — independently of the search.
- Search the current folder, or the whole device.
- One-directional tray: pick files from one side and the destination is chosen for
  you. No paths are ever typed by hand.
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
│   TCP socket (port 5000)  ◄──┼── control events
└────────────┬─────────────────┘
             │  ADB reverse — over USB or Wi-Fi
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

The desktop application listens on `127.0.0.1:5000`. `adb reverse` maps that port
into the device, so the phone connects to what it sees as its own localhost. The
transport is identical whether the link is USB or Wi-Fi, which is why enabling
Wi-Fi requires no change on the phone side.

The same socket carries control events back to the phone — TCP is full-duplex, so
no second port is needed.

Device presence comes from ADB's `host:track-devices` stream, so connections and
disconnections are detected as events rather than by polling.

File operations go through `adb push` and `adb pull`, running off the UI thread.
Media listings are read from Android's `MediaStore` index instead of walking the
filesystem, which keeps browsing instant on devices with tens of thousands of photos.

## Requirements

| | |
|---|---|
| Desktop | Windows 10+ (64-bit), or a current Linux distribution |
| Device | Android 7.0 (API 24) or later |
| Connection | USB cable for the initial setup; Wi-Fi afterwards |
| Network | Phone and PC on the same local network for wireless use |

## Installation

### Windows

Download `TransFEL-Setup-*.exe` from the
[Releases](https://github.com/pipeeex/TransFEL/releases) page and run it.

The installer bundles ADB and FFmpeg inside the application folder. Nothing is
added to the system `PATH` and no existing ADB or FFmpeg installation is modified.
A portable `.zip` is also published if you would rather not install.

### Linux

```bash
tar -xzf TransFEL-v0.1.0-linux-x86_64.tar.gz
cd TransFEL-v0.1.0-linux-x86_64
./instalar.sh
```

On Linux, TransFEL uses the system's `adb` and `ffmpeg` instead of bundling them:

```bash
sudo apt install android-tools-adb ffmpeg      # Debian / Ubuntu
sudo dnf install android-tools ffmpeg          # Fedora
sudo pacman -S android-tools ffmpeg            # Arch
```

For ADB to see your phone without `sudo`, add yourself to the right group and log
back in:

```bash
sudo usermod -aG plugdev $USER
```

### The Android application

Download `TransFEL.apk` from the same Releases page and install it on your device.
Android will ask you to allow installation from unknown sources.

### Enable USB debugging

1. Open **Settings → About phone**.
2. Tap **Build number** seven times to unlock Developer options.
3. Open **Settings → System → Developer options**.
4. Enable **USB debugging**.
5. Connect the device with a USB cable.
6. Accept the authorization prompt on the phone.

### Connect

1. Launch TransFEL on your computer. The device is detected automatically.
2. Open the TransFEL app on your phone and tap **Iniciar transmision**.
3. Grant the screen capture permission when Android asks.

The screen appears in the desktop window within a second or two.

## Going wireless

Open the **Conexion** tab on the desktop.

**With a cable connected** — press **Activar conexion WiFi**. TransFEL opens the TCP
port on the device, reads its IP address and connects. Once the log shows
`Conectado a 192.168.x.x:5555`, unplug the cable: mirroring, control and file
transfers keep working.

**Without a cable (Android 11+)** — on the phone, open **Developer options → Wireless
debugging → Pair device with pairing code**. Enter the address and code shown there
in the pairing section of the Conexion tab, then connect using the IP and port from
the main Wireless debugging screen.

Notes:

- `adb tcpip` does not survive a phone reboot; repeat the cable step, or use
  wireless debugging, which does persist.
- If the router hands out a new IP, TransFEL retries the saved address first and
  reports the failure in the log.
- Windows Firewall may prompt for `adb.exe` the first time. Allow it on private networks.

## Enabling remote control

Control needs Android's accessibility API, because no ordinary app may inject input
into other apps.

1. On the phone, open the TransFEL app.
2. In the **Control remoto** card, tap **Abrir Ajustes de Accesibilidad**.
3. Find **TransFEL - Control remoto** and turn it on.

The card turns green once it is active. On the desktop, start a broadcast and tick
the 🖱 box in the Pantalla tab; the Back, Home and Recents buttons appear next to it.

What you can do: tap, long press (hold half a second), drag in real time, and scroll
with the mouse wheel. Keyboard input is not implemented yet.

## Usage

### Mirroring

Select **Pantalla** in the sidebar. The stream starts when the phone begins
broadcasting and ends cleanly when it stops. The sidebar always reflects the current
state: active, paused, closed or idle. While it runs, a notification on the phone
offers **Pausar** and **Finalizar** without opening the app.

### Transferring files

1. Select **Archivos** in the sidebar.
2. Choose **Mi PC** or **Celular** at the top.
3. Navigate with the shortcuts, or search — either in the current folder or across
   the whole device.
4. Press **＋ Seleccionar** on the files you want. They collect in the tray on the right.
5. Pick a destination from the list and press the transfer button.

The tray works in one direction at a time. Files chosen on the phone can only go to
the PC and vice versa, so the destination and the button label are decided for you.
Destinations are preset folders — Downloads, Camera, Documents, Music, Movies on the
phone, and a `Downloads/TransFEL` folder on the PC that you can change with the
native folder picker.

## Project layout

```text
TransFEL/
├── android/                 Kotlin application (Jetpack Compose)
│   └── app/src/main/java/com/example/transfelandroid/
│       ├── MainActivity.kt          UI and state machine
│       ├── EstadoTransmision.kt     Broadcast states
│       ├── ScreenCaptureService.kt  Capture, encoding, TCP, control channel
│       ├── ControlService.kt        Gesture injection (AccessibilityService)
│       └── ui/TransfelTheme.kt      Shared color palette
├── desktop/                 Rust application
│   ├── src/
│   │   ├── main.rs          Entry point and window setup
│   │   ├── app.rs           Application state and frame loop
│   │   ├── adb.rs           ADB and FFmpeg process wrappers
│   │   ├── device.rs        Device properties
│   │   ├── watcher.rs       Connect / disconnect event stream
│   │   ├── wifi.rs          Wireless setup, pairing and reconnect
│   │   ├── stream.rs        TCP server, H.264 decoding, control channel
│   │   ├── files.rs         File manager model and workers
│   │   ├── theme.rs         Color tokens
│   │   └── ui/              Interface modules
│   └── Cargo.toml
├── installer/               Inno Setup script (Windows)
├── packaging/linux/         .desktop entry and install scripts
├── licenses/                Third-party license notices
└── .github/workflows/       Release automation
```

Configuration is stored in `%APPDATA%\TransFEL\` on Windows and
`~/.config/transfel/` on Linux.

## Building from source

### Desktop

Requires the [Rust toolchain](https://rustup.rs/) (stable).

```bash
git clone https://github.com/pipeeex/TransFEL.git
cd TransFEL/desktop
cargo build --release
```

On Linux you also need the development headers:

```bash
sudo apt install build-essential pkg-config libgtk-3-dev \
     libxkbcommon-dev libwayland-dev libxcb1-dev
```

The binary lands in `target/release/`. On Windows, place `adb.exe` and `ffmpeg.exe`
in a `tools/` folder next to it — the application resolves them relative to its own
path and falls back to the system `PATH` if they are absent, which is how it works
on Linux.

### Android

Open the `android/` folder in Android Studio and run:

```bash
./gradlew assembleRelease
```

### Releases

Pushing a tag that starts with `v` builds Windows and Linux packages and opens a
draft release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The APK is built and signed in Android Studio and uploaded to the draft by hand.

## Design

Both applications share one palette, so the phone and the desktop read as the same
product.

| Token | Hex | Use |
|---|---|---|
| Background | `#0F0F12` | Window and screen background |
| Panel | `#16161B` | Sidebars and secondary surfaces |
| Card | `#1C1C22` | Cards, rows, inputs |
| Border | `#373741` | Dividers and outlines |
| Accent | `#5A82FF` | Primary actions, selection |
| Success | `#46C878` | Live, completed |
| Warning | `#E0A33C` | Paused |
| Danger | `#E65A5A` | Errors, destructive actions |
| Muted | `#8C8C96` | Secondary text |

## Troubleshooting

**The stream freezes and nothing in the logs explains it**
Another app is capturing the same screen. Android Studio's *Running Devices* panel
is the usual culprit — the two capture sessions compete and TransFEL stops receiving
frames without any error. Close the other one. Screen recorders and video-call apps
sharing the screen cause the same thing.

**The device is not detected**
Confirm USB debugging is enabled and that you accepted the authorization prompt on
the phone. Try a different cable — charge-only cables carry no data. Then run
`adb devices` and check that your device is listed as `device`, not `unauthorized`.

**Wi-Fi connection fails to authenticate**
The device has not authorized this computer. Connect it once over USB, accept the
prompt, and enable Wi-Fi from the Conexion tab again.

**The Wi-Fi device disappeared after rebooting the phone**
`adb tcpip` is reset on reboot. Reconnect the cable and press **Activar conexion
WiFi** again, or set up wireless debugging instead.

**The screen stays black after starting the broadcast**
The screen capture permission was likely denied. Close the phone app, reopen it and
accept the prompt.

**Remote control does nothing in a particular app**
Banking apps and some others block accessibility services on purpose. This is an
Android security feature, not a bug. Control works normally everywhere else.

**Pause and stop are greyed out on the phone**
That is intentional — they only become available while a broadcast is actually running.

**Some folders on the phone appear empty**
Android 11 and later restrict access to `Android/data` and `Android/obb`. This is a
platform limitation and affects every file manager, not just TransFEL.

**Transfers fail with a permission error**
Choose a destination inside `/sdcard`. System paths are read-only over ADB without root.

## Roadmap

- [x] Wi-Fi connection for mirroring, control and file transfers
- [x] Type filters and device-wide search in the file manager
- [x] Automatic device detection
- [x] Remote control: mouse, drag and scroll
- [ ] Keyboard input, through an `InputMethodService`
- [ ] Transfer progress per file, with speed and ETA
- [ ] Drag and drop onto the application window
- [ ] Configurable bitrate and resolution
- [ ] Clipboard sync between phone and PC
- [ ] macOS build

## Uninstalling

**Windows** — remove TransFEL from *Settings → Apps*, or use the bundled uninstaller.
It removes the application, the bundled ADB and FFmpeg binaries, and the
configuration and cache files TransFEL created.

**Linux** — run `./desinstalar.sh` from the package folder.

Files you transferred to your computer are never touched, and no system-wide
software is modified or removed.

## Privacy

TransFEL runs entirely on your local network. No video, file or device information
is sent to any external server, and the application makes no network requests beyond
the direct connection between your phone and your computer.

## License

TransFEL is released under the **GNU General Public License v3.0**.
See [LICENSE](LICENSE) for the full text.

You are free to use, study, modify and redistribute it, provided that any
distributed derivative work is also licensed under the GPL and its source code is
made available.

### Bundled third-party software

The Windows build redistributes the following unmodified binaries. Their licenses
are in the `licenses/` folder of the repository and of the installation. The Linux
build bundles nothing and uses the system packages instead.

| Component | License | Source |
|---|---|---|
| FFmpeg | GPL-3.0 | [ffmpeg.org](https://ffmpeg.org) |
| Android Debug Bridge | Android SDK Terms (source: Apache-2.0) | [developer.android.com](https://developer.android.com/tools/releases/platform-tools) |

Rust dependencies are licensed under MIT or Apache-2.0.
