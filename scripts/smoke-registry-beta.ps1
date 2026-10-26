param(
  [string] $WorkDir = "",
  [int] $Port = 0
)

$ErrorActionPreference = "Stop"
$ownsWorkDir = [string]::IsNullOrWhiteSpace($WorkDir)
if ($ownsWorkDir) {
  $WorkDir = Join-Path ([IO.Path]::GetTempPath()) ("axonyx-registry-beta-" + [guid]::NewGuid().ToString("N"))
}
$WorkDir = [IO.Path]::GetFullPath($WorkDir)
$toolRoot = Join-Path $WorkDir "tools"
$appRoot = Join-Path $WorkDir "apps"
$originalPath = $env:PATH
$originalInstallRoot = $env:CARGO_INSTALL_ROOT
$originalLocation = Get-Location
$server = $null

function Invoke-CargoStep {
  param([string] $Label, [scriptblock] $Command)
  & $Command
  if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit code $LASTEXITCODE" }
}

try {
  New-Item -ItemType Directory -Path $toolRoot, $appRoot -Force | Out-Null
  $env:PATH = (Join-Path $toolRoot "bin") + [IO.Path]::PathSeparator + $originalPath
  $env:CARGO_INSTALL_ROOT = $toolRoot

  Invoke-CargoStep "install published create-axonyx" {
    cargo install create-axonyx --version 0.8.1 --locked --root $toolRoot
  }
  Invoke-CargoStep "install published cargo-axonyx" {
    cargo install cargo-axonyx --version 0.8.3 --locked --root $toolRoot
  }

  foreach ($template in @("site", "blog", "docs")) {
    $name = "beta-$template"
    Push-Location $appRoot
    try {
      Invoke-CargoStep "scaffold $template" {
        create-axonyx $name --yes --template $template --runtime-source registry
      }
    } finally { Pop-Location }

    $project = Join-Path $appRoot $name
    $manifest = Get-Content (Join-Path $project "Cargo.toml") -Raw
    if ($manifest -notmatch 'axonyx-runtime\s*=\s*\{\s*version\s*=\s*"0\.8\.1"' -or
        $manifest -notmatch 'axonyx-ui\s*=\s*"0\.0\.86"' -or
        $manifest -match '(?m)^\s*(\[patch\.|.*\b(path|git)\s*=)') {
      throw "$template scaffold did not use published runtime and UI crates"
    }

    Push-Location $project
    try {
      Invoke-CargoStep "$template check" { cargo ax check }
      Invoke-CargoStep "$template strict doctor" { cargo ax doctor --deny-warnings }
      Invoke-CargoStep "$template build" { cargo ax build --clean }
    } finally { Pop-Location }

    if (!(Test-Path (Join-Path $project "dist/index.html"))) {
      throw "$template did not produce dist/index.html"
    }
    if ($template -eq "blog" -and !(Test-Path (Join-Path $project "dist/blog/hello-axonyx/index.html"))) {
      throw "blog article was not prerendered"
    }
    if ($template -eq "docs" -and !(Test-Path (Join-Path $project "dist/getting-started/index.html"))) {
      throw "docs getting-started page was not generated"
    }
  }

  if ($Port -eq 0) {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $Port = $listener.LocalEndpoint.Port
    $listener.Stop()
  }
  $siteRoot = Join-Path $appRoot "beta-site"
  $exe = if ($IsWindows) { "cargo-axonyx.exe" } else { "cargo-axonyx" }
  $start = [Diagnostics.ProcessStartInfo]::new()
  $start.FileName = Join-Path (Join-Path $toolRoot "bin") $exe
  $start.WorkingDirectory = $siteRoot
  $start.UseShellExecute = $false
  $start.CreateNoWindow = $true
  foreach ($arg in @("run", "start", "--host", "127.0.0.1", "--port", "$Port")) {
    [void] $start.ArgumentList.Add($arg)
  }
  $server = [Diagnostics.Process]::Start($start)
  $base = "http://127.0.0.1:$Port"
  $ready = $false
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    if ($server.HasExited) { throw "site server exited with code $($server.ExitCode)" }
    try {
      $response = Invoke-WebRequest "$base/" -TimeoutSec 2 -SkipHttpErrorCheck
      if ($response.StatusCode -eq 200) { $ready = $true; break }
    } catch { }
    Start-Sleep -Milliseconds 500
  }
  if (!$ready) { throw "site server did not start" }

  foreach ($check in @(
    @{ Path = "/about"; Status = 200; Text = "About" },
    @{ Path = "/contact"; Status = 200; Text = "Contact" },
    @{ Path = "/brand-mark.svg"; Status = 200; Text = "<svg" },
    @{ Path = "/_ax/pkg/axonyx-ui/index.css"; Status = 200; Text = ".ax-" },
    @{ Path = "/beta-unknown-route"; Status = 404; Text = "" }
  )) {
    $response = Invoke-WebRequest ($base + $check.Path) -TimeoutSec 10 -SkipHttpErrorCheck
    if ($response.StatusCode -ne $check.Status -or
        ($check.Text -and !$response.Content.Contains($check.Text))) {
      throw "$($check.Path): expected HTTP $($check.Status) with '$($check.Text)', got $($response.StatusCode)"
    }
  }
  Write-Host "Registry-only Beta 1 smoke passed (site, blog, docs, HTTP, UI assets, 404)."
} finally {
  if ($server -and !$server.HasExited) {
    $server.Kill($true)
    $server.WaitForExit()
  }
  $env:PATH = $originalPath
  $env:CARGO_INSTALL_ROOT = $originalInstallRoot
  Set-Location $originalLocation
  if ($ownsWorkDir -and (Test-Path -LiteralPath $WorkDir)) {
    $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($WorkDir.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase)) {
      Remove-Item -LiteralPath $WorkDir -Recurse -Force
    }
  }
}
