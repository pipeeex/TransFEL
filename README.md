# TransFEL

TransFEL is a desktop application that allows you to connect an Android device to your computer and mirror its screen in real time.

The project consists of an Android application and a Windows desktop application.

## Features

- Detect Android devices through USB.
- Display device information.
- Capture the Android screen.
- Encode the screen using H.264.
- Transfer video through TCP.
- Decode and display the screen on the computer.
- Control the Android device from the desktop application.
- Transfer files between the Android device and the computer.

## Architecture

```text
Android Device
      |
      | MediaProjection
      v
   MediaCodec
      |
      | H.264
      v
      TCP
      |
      | ADB Reverse
      v
TransFEL Desktop
      |
      v
    FFmpeg
      |
      v
   Desktop GUI
```

## Technologies

### Android Application

- Kotlin
- Android SDK
- Jetpack Compose
- MediaProjection
- MediaCodec
- TCP Sockets

### Desktop Application

- Rust
- Cargo
- eframe
- egui
- ADB
- FFmpeg

## Installation

### For Users

The final version of TransFEL will be distributed as a Windows installer:

```text
TransFEL-Setup.exe
```

You do not need to install Rust, Android Studio, ADB, or FFmpeg manually.

#### 1. Install TransFEL

Download and run:

```text
TransFEL-Setup.exe
```

Follow the installation steps shown by the installer.

TransFEL will automatically install the components required by the application.

#### 2. Install the Android Application

Install the TransFEL Android application on your Android device.

The Android application will be provided as an APK.

#### 3. Enable USB Debugging

On the Android device:

1. Open Settings.
2. Enable Developer Options.
3. Enable USB Debugging.
4. Connect the device to the computer using a USB cable.
5. Accept the USB debugging authorization request on the phone.

#### 4. Start TransFEL

Open TransFEL from the Windows Start Menu or desktop shortcut.

The application will automatically search for the connected Android device.

Once the device is detected, TransFEL can establish the connection and start the screen mirroring process.

### Included Components

The TransFEL installer includes the components required by the desktop application.

```text
TransFEL/
├── TransFEL.exe
├── tools/
│   ├── adb.exe
│   ├── AdbWinApi.dll
│   ├── AdbWinUsbApi.dll
│   └── ffmpeg.exe
└── resources/
```

ADB and FFmpeg are installed locally with TransFEL. They do not need to be installed separately or added manually to the Windows PATH.

## Uninstallation

TransFEL can be removed using the Windows uninstaller.

The uninstaller removes the components installed by TransFEL, including:

- TransFEL
- Bundled ADB
- Bundled FFmpeg
- Application resources
- TransFEL configuration and cache files

The uninstaller does not remove software or files
