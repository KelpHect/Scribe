param(
    [string]$Executable = "target/release/scribe.exe",
    [ValidateRange(1, 100)]
    [int]$Runs = 10,
    [ValidateRange(100, 30000)]
    [int]$TimeoutMs = 5000,
    [ValidateRange(0, 60000)]
    [int]$PrimeDelayMs = 5000,
    [ValidateRange(0, 5000)]
    [int]$MemorySampleDelayMs = 250
)

$ErrorActionPreference = "Stop"

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$sampleRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("scribe-startup-" + [guid]::NewGuid().ToString("N"))
$originalAppData = $env:APPDATA
$originalLocalAppData = $env:LOCALAPPDATA
$originalStartupTrace = $env:SCRIBE_STARTUP_TRACE
$samples = [System.Collections.Generic.List[object]]::new()

New-Item -ItemType Directory -Path $sampleRoot | Out-Null

try {
    $env:APPDATA = Join-Path $sampleRoot "Roaming"
    $env:LOCALAPPDATA = Join-Path $sampleRoot "Local"
    New-Item -ItemType Directory -Force -Path $env:APPDATA, $env:LOCALAPPDATA | Out-Null

    $env:SCRIBE_STARTUP_TRACE = Join-Path $sampleRoot "prime.trace"
    $prime = Start-Process -FilePath $resolvedExecutable -PassThru
    try {
        if (-not $prime.WaitForInputIdle($TimeoutMs)) {
            throw "Scribe did not reach an idle input queue while priming the isolated profile."
        }
        if ($PrimeDelayMs -gt 0) {
            Start-Sleep -Milliseconds $PrimeDelayMs
        }
    }
    finally {
        if (-not $prime.HasExited) {
            $prime.Kill($true)
            $prime.WaitForExit()
        }
        $prime.Dispose()
    }

    for ($run = 1; $run -le $Runs; $run++) {
        $tracePath = Join-Path $sampleRoot "run-$run.trace"
        $env:SCRIBE_STARTUP_TRACE = $tracePath
        $startedAt = [System.Diagnostics.Stopwatch]::GetTimestamp()
        $process = Start-Process -FilePath $resolvedExecutable -PassThru

        try {
            if (-not $process.WaitForInputIdle($TimeoutMs)) {
                throw "Scribe did not reach an idle input queue within $TimeoutMs ms on run $run."
            }

            $elapsed = [System.Diagnostics.Stopwatch]::GetElapsedTime($startedAt).TotalMilliseconds
            if ($MemorySampleDelayMs -gt 0) {
                Start-Sleep -Milliseconds $MemorySampleDelayMs
            }
            $process.Refresh()
            $events = @{}
            if (Test-Path -LiteralPath $tracePath) {
                foreach ($line in Get-Content -LiteralPath $tracePath) {
                    $name, $microseconds = $line -split " ", 2
                    $events[$name] = [double]$microseconds / 1000
                }
            }

            $samples.Add([pscustomobject]@{
                    Run          = $run
                    InputIdle    = [math]::Round($elapsed, 1)
                    FirstFrame   = [math]::Round($events["first_frame"], 1)
                    CatalogReady = [math]::Round($events["catalog_ready"], 1)
                    WorkingMB    = [math]::Round($process.WorkingSet64 / 1MB, 1)
                    PrivateMB    = [math]::Round($process.PrivateMemorySize64 / 1MB, 1)
                })
        }
        finally {
            if (-not $process.HasExited) {
                $process.Kill($true)
                $process.WaitForExit()
            }
            $process.Dispose()
        }
    }
}
finally {
    $env:APPDATA = $originalAppData
    $env:LOCALAPPDATA = $originalLocalAppData
    $env:SCRIBE_STARTUP_TRACE = $originalStartupTrace
    Remove-Item -LiteralPath $sampleRoot -Recurse -Force
}

$ordered = @($samples.InputIdle | Sort-Object)
$middle = [int][math]::Floor($ordered.Count / 2)
$median = if ($ordered.Count % 2 -eq 0) {
    ($ordered[$middle - 1] + $ordered[$middle]) / 2
}
else {
    $ordered[$middle]
}

$samples | Format-Table -AutoSize
[pscustomobject]@{
    Executable         = $resolvedExecutable
    Runs               = $samples.Count
    ProfilePrimeMs     = $PrimeDelayMs
    FirstMeasuredMs    = $samples[0].InputIdle
    MedianInputIdleMs  = [math]::Round($median, 1)
    MeanInputIdleMs    = [math]::Round(($samples.InputIdle | Measure-Object -Average).Average, 1)
    MeanFirstFrameMs   = [math]::Round(($samples.FirstFrame | Measure-Object -Average).Average, 1)
    MeanCatalogReadyMs = [math]::Round(($samples.CatalogReady | Measure-Object -Average).Average, 1)
    MeanWorkingMB      = [math]::Round(($samples.WorkingMB | Measure-Object -Average).Average, 1)
    MeanPrivateMB      = [math]::Round(($samples.PrivateMB | Measure-Object -Average).Average, 1)
} | Format-List
