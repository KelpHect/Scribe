param(
    [string]$Executable = "target/release/scribe.exe",
    [ValidateRange(1000, 30000)]
    [int]$ReadyTimeoutMs = 10000,
    [ValidateRange(1000, 30000)]
    [int]$CloseTimeoutMs = 10000,
    [switch]$KeepProfile
)

$ErrorActionPreference = "Stop"

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    "scribe-acceptance-" + [guid]::NewGuid().ToString("N")
)
$roamingRoot = Join-Path $profileRoot "Roaming"
$localRoot = Join-Path $profileRoot "Local"
$scribeRoot = Join-Path $roamingRoot "Scribe"
$settingsPath = Join-Path $scribeRoot "settings.toml"
$databasePath = Join-Path $scribeRoot "scribe.redb"
$originalAppData = $env:APPDATA
$originalLocalAppData = $env:LOCALAPPDATA
$originalStartupTrace = $env:SCRIBE_STARTUP_TRACE

function Wait-ForTraceEvents {
    param(
        [Parameter(Mandatory)]
        [string]$TracePath,
        [Parameter(Mandatory)]
        [string[]]$Events,
        [Parameter(Mandatory)]
        [int]$TimeoutMs
    )

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if (Test-Path -LiteralPath $TracePath) {
            $observed = @(
                Get-Content -LiteralPath $TracePath |
                    ForEach-Object { ($_ -split " ", 2)[0] }
            )
            $missing = @($Events | Where-Object { $_ -notin $observed })
            if ($missing.Count -eq 0) {
                return
            }
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)

    throw "Scribe did not report all required startup events: $($Events -join ', ')."
}

function Invoke-ScribeRun {
    param(
        [Parameter(Mandatory)]
        [string]$Name
    )

    $tracePath = Join-Path $profileRoot "$Name.trace"
    $env:SCRIBE_STARTUP_TRACE = $tracePath
    $process = Start-Process -FilePath $resolvedExecutable -PassThru
    try {
        if (-not $process.WaitForInputIdle($ReadyTimeoutMs)) {
            throw "Scribe did not reach an idle input queue during $Name."
        }
        Wait-ForTraceEvents -TracePath $tracePath -Events @(
            "main_enter",
            "embedded_assets_ready",
            "gpui_run",
            "component_init",
            "window_root",
            "first_frame",
            "catalog_ready"
        ) -TimeoutMs $ReadyTimeoutMs

        $events = @{}
        foreach ($line in Get-Content -LiteralPath $tracePath) {
            $event, $microseconds = $line -split " ", 2
            $events[$event] = [double]$microseconds / 1000
        }
        $process.Refresh()
        $workingSetMB = [math]::Round($process.WorkingSet64 / 1MB, 1)
        $privateMemoryMB = [math]::Round($process.PrivateMemorySize64 / 1MB, 1)

        if (-not $process.CloseMainWindow()) {
            throw "Scribe did not accept a graceful window-close request during $Name."
        }
        if (-not $process.WaitForExit($CloseTimeoutMs)) {
            throw "Scribe did not exit within $CloseTimeoutMs ms after closing during $Name."
        }
        if ($process.ExitCode -ne 0) {
            throw "Scribe exited with code $($process.ExitCode) during $Name."
        }
    }
    finally {
        if (-not $process.HasExited) {
            # Kill() without arguments: the tree-kill overload does not exist
            # on Windows PowerShell 5.1, and Scribe spawns no child processes.
            $process.Kill()
            $process.WaitForExit()
        }
        $process.Dispose()
    }

    [pscustomobject]@{
        Name = $Name
        Trace = $tracePath
        WindowOpenedMs = [math]::Round($events["window_opened"], 1)
        CatalogReadyMs = [math]::Round($events["catalog_ready"], 1)
        FirstFrameMs = [math]::Round($events["first_frame"], 1)
        WorkingSetMB = $workingSetMB
        PrivateMemoryMB = $privateMemoryMB
    }
}

New-Item -ItemType Directory -Force -Path $roamingRoot, $localRoot | Out-Null

try {
    $env:APPDATA = $roamingRoot
    $env:LOCALAPPDATA = $localRoot

    # Seed settings before the first launch: background_alerts defaults to
    # true, which vetoes window-close and minimizes to the tray instead of
    # exiting — the graceful-close assertion below needs exit-on-close.
    New-Item -ItemType Directory -Force -Path $scribeRoot | Out-Null
    $settings = @"
addon_path = ""
auto_update = false
memory_limit_mb = 192
theme = "scribe"
background_alerts = false
"@
    [System.IO.File]::WriteAllText(
        $settingsPath,
        $settings,
        [System.Text.UTF8Encoding]::new($false)
    )

    $first = Invoke-ScribeRun -Name "first-launch"
    if (-not (Test-Path -LiteralPath $databasePath)) {
        throw "First launch did not create the isolated scribe.redb database."
    }
    if ((Get-Item -LiteralPath $databasePath).Length -le 0) {
        throw "First launch created an empty scribe.redb database."
    }

    $settingsHashBefore = (Get-FileHash -Algorithm SHA256 -LiteralPath $settingsPath).Hash
    $databaseLengthBefore = (Get-Item -LiteralPath $databasePath).Length

    $restart = Invoke-ScribeRun -Name "restart"
    $settingsHashAfter = (Get-FileHash -Algorithm SHA256 -LiteralPath $settingsPath).Hash
    $databaseHashAfter = (Get-FileHash -Algorithm SHA256 -LiteralPath $databasePath).Hash
    if ($settingsHashAfter -ne $settingsHashBefore) {
        throw "Restart unexpectedly changed the isolated settings.toml file."
    }
    if ((Get-Item -LiteralPath $databasePath).Length -le 0) {
        throw "Restart left an empty scribe.redb database."
    }

    [pscustomobject]@{
        OperatingSystem = [System.Environment]::OSVersion.VersionString
        Architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
        PowerShell = $PSVersionTable.PSVersion.ToString()
        Executable = $resolvedExecutable
        ExecutableSHA256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $resolvedExecutable).Hash
        Profile = $profileRoot
        FirstLaunchTrace = $first.Trace
        FirstLaunchWindowOpenedMs = $first.WindowOpenedMs
        FirstLaunchCatalogReadyMs = $first.CatalogReadyMs
        FirstLaunchFirstFrameMs = $first.FirstFrameMs
        FirstLaunchWorkingSetMB = $first.WorkingSetMB
        FirstLaunchPrivateMemoryMB = $first.PrivateMemoryMB
        RestartTrace = $restart.Trace
        RestartWindowOpenedMs = $restart.WindowOpenedMs
        RestartCatalogReadyMs = $restart.CatalogReadyMs
        RestartFirstFrameMs = $restart.FirstFrameMs
        RestartWorkingSetMB = $restart.WorkingSetMB
        RestartPrivateMemoryMB = $restart.PrivateMemoryMB
        SettingsSHA256 = $settingsHashAfter
        DatabaseSHA256 = $databaseHashAfter
        DatabaseBytesBeforeRestart = $databaseLengthBefore
        DatabaseBytesAfterRestart = (Get-Item -LiteralPath $databasePath).Length
        GracefulClose = $true
        Passed = $true
    } | Format-List
}
finally {
    $env:APPDATA = $originalAppData
    $env:LOCALAPPDATA = $originalLocalAppData
    $env:SCRIBE_STARTUP_TRACE = $originalStartupTrace
    if (-not $KeepProfile -and (Test-Path -LiteralPath $profileRoot)) {
        Remove-Item -LiteralPath $profileRoot -Recurse -Force
    }
}
