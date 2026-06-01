param(
  [ValidateSet("minimal", "site", "docs")]
  [string] $Template = "minimal",

  [int] $Port = 3925,

  [int] $FallbackPort = 3926,

  [string] $WorkDir = ""
)

$ErrorActionPreference = "Stop"

function Invoke-SmokeRequest {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Url,

    [int] $ExpectedStatus = 200,

    [string] $Expect = ""
  )

  $request = [System.Net.HttpWebRequest]::Create($Url)
  $request.Method = "GET"
  $request.AllowAutoRedirect = $false

  try {
    $response = [System.Net.HttpWebResponse] $request.GetResponse()
  } catch [System.Net.WebException] {
    if ($_.Exception.Response -eq $null) {
      throw
    }
    $response = [System.Net.HttpWebResponse] $_.Exception.Response
  }

  try {
    $reader = New-Object System.IO.StreamReader($response.GetResponseStream())
    $body = $reader.ReadToEnd()

    if ([int] $response.StatusCode -ne $ExpectedStatus) {
      throw "Expected HTTP $ExpectedStatus from $Url, got $([int] $response.StatusCode). Body: $body"
    }

    if (![string]::IsNullOrEmpty($Expect) -and $body -notmatch $Expect) {
      throw "Expected response from $Url to match '$Expect'. Body: $body"
    }

    return $body
  } finally {
    $response.Dispose()
  }
}

function Start-AxonyxServer {
  param(
    [Parameter(Mandatory = $true)]
    [string] $FrameworkRoot,

    [Parameter(Mandatory = $true)]
    [string] $AppRoot,

    [Parameter(Mandatory = $true)]
    [int] $Port,

    [string] $Transport = "",

    [Parameter(Mandatory = $true)]
    [string] $Stdout,

    [Parameter(Mandatory = $true)]
    [string] $Stderr
  )

  $args = @(
    "run",
    "--manifest-path",
    (Join-Path $FrameworkRoot "Cargo.toml"),
    "-p",
    "cargo-axonyx",
    "--bin",
    "cargo-axonyx",
    "--",
    "run",
    "dev",
    "--host",
    "127.0.0.1",
    "--port",
    "$Port"
  )

  if (![string]::IsNullOrEmpty($Transport)) {
    $args += @("--transport", $Transport)
  }

  return Start-Process -FilePath "cargo" -ArgumentList $args -WorkingDirectory $AppRoot -RedirectStandardOutput $Stdout -RedirectStandardError $Stderr -WindowStyle Hidden -PassThru
}

function Stop-AxonyxServer {
  param(
    [AllowNull()]
    [System.Diagnostics.Process] $Process
  )

  if ($null -ne $Process -and !$Process.HasExited) {
    Stop-Process -Id $Process.Id -Force
    $Process.WaitForExit(5000) | Out-Null
  }
}

function Wait-ForServer {
  param(
    [Parameter(Mandatory = $true)]
    [System.Diagnostics.Process] $Process,

    [Parameter(Mandatory = $true)]
    [string] $Url,

    [Parameter(Mandatory = $true)]
    [string] $Stdout,

    [Parameter(Mandatory = $true)]
    [string] $Stderr
  )

  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    Start-Sleep -Milliseconds 300
    try {
      Invoke-SmokeRequest -Url $Url -ExpectedStatus 200 -Expect "Axonyx" | Out-Null
      return
    } catch {
      if ($Process.HasExited) {
        throw "Server exited early. stdout: $(Get-Content -LiteralPath $Stdout -Raw) stderr: $(Get-Content -LiteralPath $Stderr -Raw)"
      }
    }
  }

  throw "Server did not respond at $Url"
}

$frameworkRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrEmpty($WorkDir)) {
  $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ("axonyx-server-transport-smoke-" + [guid]::NewGuid().ToString("N"))
}

$appRoot = Join-Path $WorkDir "transport-smoke"
$defaultStdout = Join-Path $WorkDir "tokio-default.stdout.log"
$defaultStderr = Join-Path $WorkDir "tokio-default.stderr.log"
$fallbackStdout = Join-Path $WorkDir "std-fallback.stdout.log"
$fallbackStderr = Join-Path $WorkDir "std-fallback.stderr.log"

Write-Host "Axonyx server transport smoke"
Write-Host "  template:      $Template"
Write-Host "  tokio port:    $Port"
Write-Host "  std port:      $FallbackPort"
Write-Host "  workdir:       $WorkDir"

if (Test-Path -LiteralPath $WorkDir) {
  Remove-Item -LiteralPath $WorkDir -Recurse -Force
}
New-Item -ItemType Directory -Path $WorkDir | Out-Null

try {
  Push-Location $WorkDir
  try {
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p create-axonyx --bin create-axonyx -- transport-smoke --yes --template $Template --runtime-source path
  } finally {
    Pop-Location
  }

  Push-Location $appRoot
  try {
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --bin cargo-axonyx -- check
  } finally {
    Pop-Location
  }

  $defaultServer = Start-AxonyxServer -FrameworkRoot $frameworkRoot -AppRoot $appRoot -Port $Port -Stdout $defaultStdout -Stderr $defaultStderr
  try {
    $defaultBaseUrl = "http://127.0.0.1:$Port"
    Wait-ForServer -Process $defaultServer -Url "$defaultBaseUrl/" -Stdout $defaultStdout -Stderr $defaultStderr
    Invoke-SmokeRequest -Url "$defaultBaseUrl/__axonyx/version?path=%2F" -ExpectedStatus 200 -Expect ".+" | Out-Null

    $defaultLog = Get-Content -LiteralPath $defaultStdout -Raw
    if ($defaultLog -notmatch "using tokio transport") {
      throw "Expected default dev server to use tokio transport. Log: $defaultLog"
    }
  } finally {
    Stop-AxonyxServer -Process $defaultServer
  }

  $fallbackServer = Start-AxonyxServer -FrameworkRoot $frameworkRoot -AppRoot $appRoot -Port $FallbackPort -Transport "std" -Stdout $fallbackStdout -Stderr $fallbackStderr
  try {
    $fallbackBaseUrl = "http://127.0.0.1:$FallbackPort"
    Wait-ForServer -Process $fallbackServer -Url "$fallbackBaseUrl/" -Stdout $fallbackStdout -Stderr $fallbackStderr

    $fallbackLog = Get-Content -LiteralPath $fallbackStdout -Raw
    if ($fallbackLog -notmatch "using std transport") {
      throw "Expected fallback dev server to use std transport. Log: $fallbackLog"
    }
  } finally {
    Stop-AxonyxServer -Process $fallbackServer
  }

  Write-Host "Axonyx server transport smoke passed."
  Write-Host "  default:  tokio"
  Write-Host "  fallback: std"
} finally {
  if (Test-Path -LiteralPath $WorkDir) {
    Remove-Item -LiteralPath $WorkDir -Recurse -Force
  }
}
