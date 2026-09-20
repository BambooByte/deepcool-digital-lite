$ErrorActionPreference = 'Stop'

$displayName = 'Deep Cool Display Service'
$helperName = 'Deep Cool Helper Service'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    try {
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`""
        )
    } catch {
        Write-Host "Administrator access was not granted: $_" -ForegroundColor Red
        Read-Host 'Press Enter to close'
    }
    exit
}

try {
    $display = Get-Service -Name $displayName
    if ($display.Status -eq 'Running') {
        foreach ($name in @($displayName, $helperName)) {
            $service = Get-Service -Name $name
            if ($service.Status -ne 'Stopped') {
                Stop-Service -Name $name
                $service.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(15))
            }
        }
        Write-Host 'Official DeepCool services are now stopped.' -ForegroundColor Green
    } else {
        foreach ($name in @($helperName, $displayName)) {
            $service = Get-Service -Name $name
            if ($service.Status -ne 'Running') {
                Start-Service -Name $name
                $service.WaitForStatus('Running', [TimeSpan]::FromSeconds(15))
            }
        }
        Write-Host 'Official DeepCool services are now running.' -ForegroundColor Green
    }

    Get-Service -Name $displayName, $helperName | Format-Table DisplayName, Status -AutoSize
} catch {
    Write-Host "Could not switch DeepCool services: $_" -ForegroundColor Red
} finally {
    Read-Host 'Press Enter to close'
}
