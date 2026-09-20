# DeepCool Digital Lite

English | [简体中文](README.zh-CN.md)

A small Rust tray application for the DeepCool CH270 DIGITAL case display.
It shows CPU or GPU temperature, power, usage, and a secondary metric. MIX
rotates between the CPU and GPU pages.

<img src="docs/app-ui.png" alt="DeepCool Digital Lite Case Display settings" width="600">

## Why this exists

The official DeepCool software supports the hardware, but it runs a
multi-process desktop app and background services. On my system, its memory
footprint was the main reason I wanted a smaller replacement. DeepCool Digital
Lite focuses on the CH270 display with a small Rust tray app and a bundled
sensor bridge.

- **Open source:** the application and sensor bridge can be inspected and
  rebuilt from this repository.
- **Small footprint:** the focused feature set uses substantially less memory
  on the system where it was developed.
- **Local operation:** the application contains no telemetry or network request
  code; sensor readings stay on the PC and are sent only to the case display.

| Same-PC snapshot | Official DeepCool | DeepCool Digital Lite |
| --- | ---: | ---: |
| Processes | 8 | 2 |
| Working set memory | 692 MB | 38 MB |
| Private memory | 492 MB | 20 MB |

The official measurement includes six app processes and two services. The Lite
measurement includes the Rust app and Sensor Bridge. These measurements were
taken on the same Windows PC at different times, so they are snapshots rather
than a general benchmark.

## Compatibility

DeepCool Digital Lite has only been tested with the **DeepCool CH270 DIGITAL**
case display. Other DeepCool digital products have not been tested, and their
compatibility cannot be guaranteed.

GPU power readings are currently unavailable, so GPU and MIX modes display
`0 W`. Support may be added in a future update.

## Runtime package

Use the complete `dist` folder. `deepcool-digital-lite.exe` requires the
bundled `sensor-bridge` directory beside it. The bridge uses
LibreHardwareMonitor to read sensors, so Fan Control is not required.

The official DeepCool display services must be stopped while Lite controls the
screen. `dist\Toggle-DeepCool-Services.cmd` toggles those services without
changing their startup type.

## Build

Double-click `Build-Release.cmd`, or run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\Build-Release.ps1
```

The script rebuilds `SensorBridge.exe` from source, builds the Rust release
executable, and refreshes the required runtime files under `dist`.

## Settings

Display settings are stored in `%APPDATA%\DeepCoolDigitalLite`. The
`Start with Windows` option creates the `DeepCoolDigitalLite` task in Windows
Task Scheduler and launches the current executable with `--minimized` at logon.

## License

<a href="https://slint.dev/"><img src="https://github.com/slint-ui/madewithslint/raw/main/assets/img/MadeWithSlint-logo-light.svg#gh-light-mode-only" alt="Made with Slint" width="120"></a>

Project code is licensed under the [MIT License](LICENSE). Bundled dependency
licenses are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

This project is based on
[deepcool-digital-windows](https://github.com/sukualam/deepcool-digital-windows)
by sukualam. The original copyright notice is preserved in the license.
