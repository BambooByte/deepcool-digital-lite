$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$releaseExe = Join-Path $projectRoot "target\release\deepcool-digital-lite.exe"
$distDirectory = Join-Path $projectRoot "dist"
$distExe = Join-Path $distDirectory "deepcool-digital-lite.exe"
$bridgeDependencies = Join-Path $projectRoot "sensor-bridge\bin"
$bridgeSourceFile = Join-Path $projectRoot "sensor-bridge\SensorBridge.cs"
$bridgeBuildDirectory = Join-Path $projectRoot "target\sensor-bridge"
$bridgeExe = Join-Path $bridgeBuildDirectory "SensorBridge.exe"
$lhmAssembly = Join-Path $bridgeDependencies "LibreHardwareMonitorLib.dll"
$bridgeLicenses = Join-Path $projectRoot "sensor-bridge\licenses"
$bridgeDestination = Join-Path $distDirectory "sensor-bridge"

Push-Location $projectRoot
try {
    if (Test-Path -LiteralPath $distExe) {
        try {
            $stream = [System.IO.File]::Open($distExe, "Open", "ReadWrite", "None")
            $stream.Dispose()
        }
        catch {
            throw "Close DeepCool Digital Lite before building the release."
        }
    }

    $csc = @(
        (Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\csc.exe")
        (Join-Path $env:WINDIR "Microsoft.NET\Framework\v4.0.30319\csc.exe")
    ) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $csc) {
        throw "The Windows .NET Framework C# compiler was not found."
    }

    New-Item -ItemType Directory -Path $bridgeBuildDirectory -Force | Out-Null
    & $csc /nologo /optimize+ /target:exe /platform:anycpu "/out:$bridgeExe" "/reference:$lhmAssembly" $bridgeSourceFile
    if ($LASTEXITCODE -ne 0) {
        throw "SensorBridge build failed with exit code $LASTEXITCODE."
    }

    cargo build --release --bin deepcool-digital-lite
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo release build failed with exit code $LASTEXITCODE."
    }

    New-Item -ItemType Directory -Path $distDirectory -Force | Out-Null
    Copy-Item -LiteralPath $releaseExe -Destination $distExe -Force
    New-Item -ItemType Directory -Path $bridgeDestination -Force | Out-Null
    Copy-Item -Path (Join-Path $bridgeDependencies "*") -Destination $bridgeDestination -Recurse -Force
    Copy-Item -LiteralPath $bridgeExe -Destination $bridgeDestination -Force
    New-Item -ItemType Directory -Path (Join-Path $bridgeDestination "licenses") -Force | Out-Null
    Copy-Item -Path (Join-Path $bridgeLicenses "*") -Destination (Join-Path $bridgeDestination "licenses") -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot "scripts\Toggle-DeepCool-Services.cmd") -Destination $distDirectory -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot "scripts\Toggle-DeepCool-Services.ps1") -Destination $distDirectory -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot "LICENSE") -Destination $distDirectory -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot "THIRD-PARTY-NOTICES.md") -Destination $distDirectory -Force

    Get-Item -LiteralPath $distExe | Select-Object FullName, Length, LastWriteTime
    Get-FileHash -LiteralPath $distExe -Algorithm SHA256
}
finally {
    Pop-Location
}
