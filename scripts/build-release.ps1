param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$gitVersion = if (git describe --tags --always 2>$null) { git describe --tags --always 2>$null } else { "dev" }
$gitCommit  = if (git rev-parse --short HEAD 2>$null) { git rev-parse --short HEAD 2>$null } else { "none" }
$buildDate  = Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ"

$windowsGuiFlag = if ((go env GOOS) -eq "windows") { "-H windowsgui " } else { "" }
$ldflags = "-s -w ${windowsGuiFlag}-X main.version=$gitVersion -X main.commit=$gitCommit -X main.date=$buildDate"

if ($env:SCRIBE_PGO_PROFILE) {
  if (-not (Test-Path $env:SCRIBE_PGO_PROFILE)) {
    Write-Host "SCRIBE_PGO_PROFILE does not exist: $env:SCRIBE_PGO_PROFILE" -ForegroundColor Red
    exit 1
  }
  $env:GOFLAGS = "$env:GOFLAGS -pgo=$env:SCRIBE_PGO_PROFILE".Trim()
  Write-Host "using Go PGO profile: $env:SCRIBE_PGO_PROFILE" -ForegroundColor Cyan
}

Write-Host "building Scribe $gitVersion ($gitCommit) $buildDate" -ForegroundColor Cyan

$buildArgs = @(
  'task',
  'build',
  "LD_FLAGS=$ldflags"
)

if ((go env GOOS) -eq "linux") {
  $buildArgs += "EXTRA_TAGS=gtk3"
}

if ($ExtraArgs) {
  $buildArgs += $ExtraArgs
}

wails3 @buildArgs
if ($LASTEXITCODE -ne 0) {
  Write-Host "build failed" -ForegroundColor Red
  exit $LASTEXITCODE
}

$exePath = Join-Path $PSScriptRoot "..\bin\Scribe.exe"
if (Test-Path $exePath) {
  $size = (Get-Item $exePath).Length / 1MB
  Write-Host "built: $exePath ($([math]::Round($size, 1)) MB)" -ForegroundColor Green
}

if (Get-Command upx -ErrorAction SilentlyContinue) {
  Write-Host "compressing with upx..." -ForegroundColor Yellow
  upx --best --lzma $exePath
  $sizeAfter = (Get-Item $exePath).Length / 1MB
  Write-Host "compressed: $([math]::Round($sizeAfter, 1)) MB" -ForegroundColor Green
}

Write-Host "done" -ForegroundColor Green
