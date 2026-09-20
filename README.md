# DeepCool Digital Lite

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)

A small Rust tray application for the DeepCool CH270 DIGITAL case display.
It shows CPU or GPU temperature, power, usage, and a secondary metric. MIX
rotates between the CPU and GPU pages.

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

Project code is licensed under the [MIT License](LICENSE). Bundled dependency
licenses are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

This project is based on
[deepcool-digital-windows](https://github.com/sukualam/deepcool-digital-windows)
by sukualam. The original copyright notice is preserved in the license.
