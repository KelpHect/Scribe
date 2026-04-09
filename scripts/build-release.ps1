param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$gitVersion = if (git describe --tags --always 2>$null) { git describe --tags --always 2>$null } else { "dev" }
$gitCommit  = if (git rev-parse --short HEAD 2>$null) { git rev-parse --short HEAD 2>$null } else { "none" }
$buildDate  = Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ"

$ldflags = "-s -w -X main.version=$gitVersion -X main.commit=$gitCommit -X main.date=$buildDate"

Write-Host "building Scribe $gitVersion ($gitCommit) $buildDate" -ForegroundColor Cyan

$buildArgs = @(
  'build',
  '-trimpath',
  '-ldflags',
  $ldflags
)

if ($ExtraArgs) {
  $buildArgs += $ExtraArgs
}

wails @buildArgs
if ($LASTEXITCODE -ne 0) {
  Write-Host "build failed" -ForegroundColor Red
  exit $LASTEXITCODE
}

$exePath = Join-Path $PSScriptRoot "..\build\bin\Scribe.exe"
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
