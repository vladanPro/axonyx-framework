param(
  [ValidateSet("minimal", "site", "docs")]
  [string] $Template = "site",

  [int] $Port = 3917,

  [string] $WorkDir = ""
)

$ErrorActionPreference = "Stop"

$frameworkRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$ownsWorkDir = $false

if ([string]::IsNullOrWhiteSpace($WorkDir)) {
  $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ("axonyx-production-server-smoke-" + [guid]::NewGuid().ToString("N"))
  $ownsWorkDir = $true
}

$WorkDir = [System.IO.Path]::GetFullPath($WorkDir)
$appName = "production-smoke"
$appRoot = Join-Path $WorkDir $appName
$originalLocation = Get-Location
$serverProcess = $null
$stdout = Join-Path $WorkDir "server.out.log"
$stderr = Join-Path $WorkDir "server.err.log"

Write-Host "Axonyx production server smoke"
Write-Host "  template: $Template"
Write-Host "  port:     $Port"
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

  $args = @(
    "run",
    "--manifest-path",
    (Join-Path $frameworkRoot "Cargo.toml"),
    "-p",
    "cargo-axonyx",
    "--bin",
    "cargo-axonyx",
    "--",
    "run",
    "start",
    "--production-server",
    "--host",
    "127.0.0.1",
    "--port",
    "$Port"
  )

  $serverProcess = Start-Process -FilePath "cargo" -ArgumentList $args -WorkingDirectory $appRoot -RedirectStandardOutput $stdout -RedirectStandardError $stderr -WindowStyle Hidden -PassThru

  $url = "http://127.0.0.1:$Port/"
  $response = $null
  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    Start-Sleep -Milliseconds 300
    try {
      $response = Invoke-WebRequest -UseBasicParsing $url
      break
    } catch {
      if ($serverProcess.HasExited) {
        throw "Production server exited early. stdout: $(Get-Content -LiteralPath $stdout -Raw) stderr: $(Get-Content -LiteralPath $stderr -Raw)"
      }
    }
  }

  if ($null -eq $response) {
    throw "Production server did not respond at $url"
  }

  if ($response.StatusCode -ne 200) {
    throw "Expected HTTP 200 from $url, got $($response.StatusCode)"
  }

  if ($response.Content -notmatch "Axonyx") {
    throw "Expected response body to contain Axonyx"
  }

  $serverLog = Get-Content -LiteralPath $stdout -Raw
  if ($serverLog -notmatch "Production server preview is enabled") {
    throw "Expected server log to confirm production server preview"
  }

  if ($serverLog -notmatch "using tokio transport") {
    throw "Expected server log to use tokio transport"
  }

  Write-Host "Axonyx production server smoke passed."
  Write-Host "  url: $url"
} finally {
  if ($null -ne $serverProcess -and -not $serverProcess.HasExited) {
    Stop-Process -Id $serverProcess.Id
    $serverProcess.WaitForExit(5000) | Out-Null
  }

  Set-Location $originalLocation

  if ($ownsWorkDir -and (Test-Path -LiteralPath $WorkDir)) {
    $resolved = Resolve-Path $WorkDir
    $temp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if ($resolved.Path.StartsWith($temp)) {
      Remove-Item -LiteralPath $resolved.Path -Recurse -Force
    }
  }
}
