param(
  [ValidateSet("minimal", "site", "docs")]
  [string] $Template = "site",

  [string] $WorkDir = ""
)

$ErrorActionPreference = "Stop"

$frameworkRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$ownsWorkDir = $false

if ([string]::IsNullOrWhiteSpace($WorkDir)) {
  $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ("axonyx-core-loop-" + [guid]::NewGuid().ToString("N"))
  $ownsWorkDir = $true
}

$WorkDir = [System.IO.Path]::GetFullPath($WorkDir)
$appName = "golden-site"
$appRoot = Join-Path $WorkDir $appName
$originalLocation = Get-Location

Write-Host "Axonyx core loop smoke"
Write-Host "  template: $Template"
Write-Host "  workdir:  $WorkDir"

New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null

try {
  try {
    Push-Location $WorkDir
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p create-axonyx -- $appName --yes --template $Template --runtime-source path
  } finally {
    Pop-Location
  }

  try {
    Push-Location $appRoot
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --bin cargo-axonyx -- check
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --bin cargo-axonyx -- doctor --deny-warnings
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --bin cargo-axonyx -- build --clean
  } finally {
    Pop-Location
  }

  $index = Join-Path $appRoot "dist/index.html"
  if (!(Test-Path -LiteralPath $index)) {
    throw "Expected static output was not generated: $index"
  }

  Write-Host "Axonyx core loop smoke passed."
  Write-Host "  output: $index"
} finally {
  Set-Location $originalLocation

  if ($ownsWorkDir -and (Test-Path -LiteralPath $WorkDir)) {
    $resolved = Resolve-Path $WorkDir
    $temp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if ($resolved.Path.StartsWith($temp)) {
      Remove-Item -LiteralPath $resolved.Path -Recurse -Force
    }
  }
}
