**Languages:** [简体中文](../README.md) · English

# YanRain OTA ToolBox

<img src="../YanRainOTAToolBox/src-tauri/icons/icon.png" width="112" alt="YanRain OTA ToolBox Logo">

An all-in-one visual toolbox for Android devices: flashing, rooting, screen mirroring, and firmware management, fully GUI-driven on Windows.

[![Release](https://img.shields.io/badge/Release-v1.1.2-e56565)](https://github.com/RegularsYr7/YanRainOTAToolBox/releases)
[![Website](https://img.shields.io/badge/Website-rainyweb.cn-e56565?logo=googlechrome&logoColor=white)](https://www.rainyweb.cn/)
[![Telegram](https://img.shields.io/badge/Telegram-%40rainyotatoolbox-26A5E4?logo=telegram&logoColor=white)](https://t.me/rainyotatoolbox)
[![License](https://img.shields.io/badge/License-GPL--3.0-e56565)](../LICENSE)

## Support and Sponsorship

If this toolbox has helped you, consider buying me a cup of tea. Development, testing, and ongoing maintenance all take considerable time, and your support is my biggest motivation to keep improving.

Every contribution counts, no matter how small. Thank you to every supporter!

| AFDian | Alipay | WeChat |
| --- | --- | --- |
| <img src="../assets/sponsor/afdian.jpg" width="240" alt="AFDian"> | <img src="../assets/sponsor/alipay.jpg" width="240" alt="Alipay"> | <img src="../assets/sponsor/wechat.png" width="240" alt="WeChat"> |

## Features

### 1. Device Management

- Auto-detect USB / wireless Android devices
- Real-time battery, storage, and memory monitoring
- One-click wired-to-wireless debugging

### 2. Flash Tools

- **Basic Flash** — Fastboot single-partition flashing, OEM unlock / lock
- **Batch Flash** — TXT script parsing, ADB Sideload, Fastboot Update
- **Partition Manager** — Format, extract, backup, full device backup
- **Deep Flash** — Qualcomm EDL 9008 mode full flashing
- **Xiaomi Assistant** — ROM search and download, HyperOS data source, one-click online flash
- **OnePlus Flash** — ROM decryption, Super merge, EDL full flashing

### 3. Root Patching

- Supports Magisk / KernelSU / KernelSU-Next / APatch
- Offline boot image patching on the desktop, no phone-side manager required

### 4. App Manager

Install, uninstall, freeze, and export apps with search, filtering, and batch operations.

### 5. Module Manager

Manage Magisk modules visually: install, enable, disable, and delete.

### 6. Screen Mirror

Integrated scrcpy for screen mirroring, key simulation, and screenshots.

### 7. File Transfer

Browse device files, upload and download with live progress.

### 8. Payload Extractor

Parse and extract payload.bin from local files or remote URLs using HTTP Range.

### 9. Download Center

Built-in aria2 multi-protocol engine for HTTP, BitTorrent, magnet links, and resumable downloads.

### 10. Firmware Center

Aggregate multi-brand ROM resources with Lanzou link parsing and one-click downloads.

### 11. Device Tweaks

Adjust resolution, DPI, battery, animation, and status bar settings.

### 12. Themes and Languages

- Light / warm / dark / system themes
- Chinese and English UI

## Download

Get the latest build from [Releases](https://github.com/RegularsYr7/YanRainOTAToolBox/releases):

| Architecture | Installer              | Portable              |
| ------------ | ---------------------- | --------------------- |
| x64          | `_x64-setup.exe`     | `_x64_portable.zip` |

> Windows only for now. Install to an ASCII-only path; non-ASCII paths may cause flashing features to fail.

## Build

Requirements: Windows 10/11, Node.js 20+, pnpm, stable Rust. Run inside `YanRainOTAToolBox/`:

```powershell
pnpm install
pnpm tauri:dev
pnpm tauri:build
```

## FAQ

**Q: Device shows "Unauthorized" after connecting?**
Tap "Allow USB debugging" on the phone. If no prompt appears: revoke authorization → reconnect → authorize again.

**Q: Wireless debugging cannot connect?**
Make sure the phone and computer are on the same LAN. Connect via USB first; the tool switches to wireless mode automatically.

**Q: Fastboot says "device locked"?**
The bootloader is locked. Use the OEM unlock feature first.

**Q: KernelSU patch reports "KMI not matched"?**
The device kernel version is not in the preset list. Specify the `.ko` file manually.

**Q: scrcpy mirror is black?**
On MIUI / HyperOS, enable "USB debugging (Security settings)", or lower the resolution / frame rate.

## Links

- Feedback: [Issues](https://github.com/RegularsYr7/YanRainOTAToolBox/issues)
- CoolAPK: [@烟雨ovo](http://www.coolapk.com/u/17411754)
- Docs: [docs](../docs)

## License

This project is licensed under the [GNU General Public License v3.0](../LICENSE) (GPL-3.0).
