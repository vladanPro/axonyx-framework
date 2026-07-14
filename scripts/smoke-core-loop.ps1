param(
  [ValidateSet("minimal", "site", "blog", "docs")]
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

  if ($Template -eq "site") {
    $about = Join-Path $appRoot "dist/about/index.html"
    $contact = Join-Path $appRoot "dist/contact/index.html"
    if (!(Test-Path -LiteralPath $about) -or !(Test-Path -LiteralPath $contact)) {
      throw "Expected site about/contact route output"
    }
  }

  if ($Template -eq "blog") {
    $article = Join-Path $appRoot "dist/blog/hello-axonyx/index.html"
    $manifest = Join-Path $appRoot "dist/_ax/content/manifest.json"
    if (!(Test-Path -LiteralPath $article)) {
      throw "Expected blog article to be prerendered: $article"
    }
    if (!(Test-Path -LiteralPath $manifest)) {
      throw "Expected blog content manifest: $manifest"
    }
    $articleHtml = Get-Content -LiteralPath $article -Raw
    if (!$articleHtml.Contains("Hello from the Axonyx workbench")) {
      throw "Expected prerendered blog article content"
    }
  }

  if ($Template -eq "docs") {
    $gettingStarted = Join-Path $appRoot "dist/getting-started/index.html"
    $reference = Join-Path $appRoot "dist/reference/index.html"
    if (!(Test-Path -LiteralPath $gettingStarted) -or !(Test-Path -LiteralPath $reference)) {
      throw "Expected docs route output"
    }
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
