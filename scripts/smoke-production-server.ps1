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

function Invoke-SmokeRequest {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Url,

    [string] $Method = "GET",

    [int] $ExpectedStatus = 200,

    [string] $Body = "",

    [string] $ContentType = "application/x-www-form-urlencoded",

    [hashtable] $Headers = @{},

    [string] $Expect = "",

    [string] $ExpectHeader = "",

    [string] $ExpectHeaderValue = ""
  )

  $request = [System.Net.HttpWebRequest]::Create($Url)
  $request.Method = $Method
  $request.AllowAutoRedirect = $false
  $request.ServicePoint.Expect100Continue = $false
  foreach ($header in $Headers.GetEnumerator()) {
    if ($header.Key -ieq "Accept") {
      $request.Accept = [string] $header.Value
    } else {
      $request.Headers[$header.Key] = [string] $header.Value
    }
  }

  if (![string]::IsNullOrEmpty($Body)) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Body)
    $request.ContentType = $ContentType
    $request.ContentLength = $bytes.Length
    $stream = $request.GetRequestStream()
    try {
      $stream.Write($bytes, 0, $bytes.Length)
    } finally {
      $stream.Dispose()
    }
  }

  $response = $null
  try {
    $response = $request.GetResponse()
  } catch [System.Net.WebException] {
    if ($_.Exception.Response -eq $null) {
      throw
    }
    $response = $_.Exception.Response
  }

  try {
    $status = [int] $response.StatusCode
    if ($status -ne $ExpectedStatus) {
      throw "Expected HTTP $ExpectedStatus from $Url, got $status"
    }

    $text = ""
    if ($Method -ne "HEAD") {
      $reader = New-Object System.IO.StreamReader($response.GetResponseStream())
      try {
        $text = $reader.ReadToEnd()
      } finally {
        $reader.Dispose()
      }
    }

    if (![string]::IsNullOrEmpty($Expect) -and $text -notmatch $Expect) {
      throw "Expected response body from $Url to match '$Expect'"
    }

    if (![string]::IsNullOrEmpty($ExpectHeader)) {
      $actual = $response.Headers[$ExpectHeader]
      if ([string]::IsNullOrEmpty($actual)) {
        throw "Expected response header '$ExpectHeader' from $Url"
      }
      if (![string]::IsNullOrEmpty($ExpectHeaderValue) -and $actual -notmatch $ExpectHeaderValue) {
        throw "Expected response header '$ExpectHeader' from $Url to match '$ExpectHeaderValue', got '$actual'"
      }
    }

    return @{
      Status = $status
      Body = $text
      Headers = $response.Headers
    }
  } finally {
    $response.Dispose()
  }
}

function Invoke-TemplateRouteChecks {
  param(
    [Parameter(Mandatory = $true)]
    [string] $BaseUrl,

    [Parameter(Mandatory = $true)]
    [string] $Template
  )

  if ($Template -eq "docs") {
    Invoke-SmokeRequest -Url "$BaseUrl/getting-started" -ExpectedStatus 200 -Expect "Getting Started|getting started" | Out-Null
    Invoke-SmokeRequest -Url "$BaseUrl/feedback" -ExpectedStatus 200 -Expect "Feedback" | Out-Null
    Invoke-SmokeCurlRequest -Url "$BaseUrl/__axonyx/action?path=%2Ffeedback&name=SendFeedback" -Method "POST" -Body "name=Smoke&message=Great+docs&tone=idea" -Headers @("Accept: application/ax-patch+json", "Content-Type: application/x-www-form-urlencoded") -ExpectedStatus 200 -Expect "feedbackStatus|idea" | Out-Null
    return
  }

  Invoke-SmokeRequest -Url "$BaseUrl/posts" -ExpectedStatus 200 -Expect "Posts" | Out-Null
  Invoke-SmokeRequest -Url "$BaseUrl/api/posts" -ExpectedStatus 200 -ExpectHeader "Content-Type" -ExpectHeaderValue "application/json" | Out-Null
  Invoke-SmokeRequest -Url "$BaseUrl/api/posts" -Method "POST" -Body "title=Smoke+Post&featured=true" -ExpectedStatus 200 -Expect "Smoke Post" | Out-Null
  Invoke-SmokeCurlRequest -Url "$BaseUrl/__axonyx/action?path=%2Fposts&name=CreatePost" -Method "POST" -Body "title=Smoke+Action&excerpt=Action+body&status=review" -Headers @("Accept: application/ax-patch+json", "Content-Type: application/x-www-form-urlencoded") -ExpectedStatus 200 -Expect "draftStatus|review" | Out-Null
}

function Invoke-SmokeCurlRequest {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Url,

    [string] $Method = "GET",

    [int] $ExpectedStatus = 200,

    [string] $Body = "",

    [string[]] $Headers = @(),

    [string] $Expect = ""
  )

  $args = @("-sS", "-i", "-X", $Method)
  foreach ($header in $Headers) {
    $args += @("-H", $header)
  }
  if (![string]::IsNullOrEmpty($Body)) {
    $args += @("--data", $Body)
  }
  $args += $Url

  $raw = & curl.exe @args
  if ($LASTEXITCODE -ne 0) {
    throw "curl failed for $Url with exit code $LASTEXITCODE"
  }

  $text = ($raw -join "`n")
  if ($text -notmatch "^HTTP/\S+\s+$ExpectedStatus\b") {
    throw "Expected HTTP $ExpectedStatus from $Url, got response:`n$text"
  }
  if (![string]::IsNullOrEmpty($Expect) -and $text -notmatch $Expect) {
    throw "Expected curl response from $Url to match '$Expect'"
  }

  return $text
}

function Invoke-OversizedHeaderSmoke {
  param(
    [Parameter(Mandatory = $true)]
    [string] $HostName,

    [Parameter(Mandatory = $true)]
    [int] $Port
  )

  $client = New-Object System.Net.Sockets.TcpClient
  $client.Connect($HostName, $Port)
  try {
    $stream = $client.GetStream()
    $writer = New-Object System.IO.StreamWriter($stream, [System.Text.Encoding]::ASCII)
    $writer.NewLine = "`r`n"
    $writer.WriteLine("POST /__axonyx/health HTTP/1.1")
    $writer.WriteLine("Host: $HostName")
    $writer.WriteLine("Content-Type: application/x-www-form-urlencoded")
    $writer.WriteLine("Content-Length: 1048608")
    $writer.WriteLine("")
    $writer.Flush()

    $reader = New-Object System.IO.StreamReader($stream)
    $raw = $reader.ReadToEnd()
    if ($raw -notmatch "^HTTP/1\.1 413\b") {
      throw "Expected oversized header smoke to receive HTTP 413, got:`n$raw"
    }
    if ($raw -notmatch "Payload Too Large") {
      throw "Expected oversized header smoke to mention Payload Too Large"
    }
  } finally {
    $client.Dispose()
  }
}

function Invoke-ChunkedPostSmoke {
  param(
    [Parameter(Mandatory = $true)]
    [string] $HostName,

    [Parameter(Mandatory = $true)]
    [int] $Port
  )

  $client = New-Object System.Net.Sockets.TcpClient
  $client.ReceiveTimeout = 5000
  $client.Connect($HostName, $Port)
  try {
    $stream = $client.GetStream()
    $writer = New-Object System.IO.StreamWriter($stream, [System.Text.Encoding]::ASCII)
    $writer.NewLine = "`r`n"
    $writer.WriteLine("POST /api/posts HTTP/1.1")
    $writer.WriteLine("Host: $HostName")
    $writer.WriteLine("Content-Type: application/x-www-form-urlencoded")
    $writer.WriteLine("Transfer-Encoding: chunked")
    $writer.WriteLine("Connection: close")
    $writer.WriteLine("")
    $writer.WriteLine("12")
    $writer.WriteLine("title=Chunked+Post")
    $writer.WriteLine("e")
    $writer.WriteLine("&featured=true")
    $writer.WriteLine("0")
    $writer.WriteLine("")
    $writer.Flush()

    $reader = New-Object System.IO.StreamReader($stream)
    $raw = $reader.ReadToEnd()
    if ($raw -notmatch "^HTTP/1\.1 200\b") {
      throw "Expected chunked POST smoke to receive HTTP 200, got:`n$raw"
    }
    if ($raw -notmatch "Chunked Post") {
      throw "Expected chunked POST smoke response to include created post title"
    }
  } finally {
    $client.Dispose()
  }
}

function Invoke-MalformedRequestSmoke {
  param(
    [Parameter(Mandatory = $true)]
    [string] $HostName,

    [Parameter(Mandatory = $true)]
    [int] $Port
  )

  $client = New-Object System.Net.Sockets.TcpClient
  $client.ReceiveTimeout = 5000
  $client.Connect($HostName, $Port)
  try {
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::ASCII.GetBytes("NOT HTTP`r`n`r`n")
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()

    $reader = New-Object System.IO.StreamReader($stream)
    $raw = $reader.ReadToEnd()
    if ($raw -notmatch "^HTTP/1\.1 400\b") {
      throw "Expected malformed request smoke to receive HTTP 400, got:`n$raw"
    }
  } finally {
    $client.Dispose()
  }
}

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
    "--host",
    "127.0.0.1",
    "--port",
    "$Port"
  )

  $serverProcess = Start-Process -FilePath "cargo" -ArgumentList $args -WorkingDirectory $appRoot -RedirectStandardOutput $stdout -RedirectStandardError $stderr -WindowStyle Hidden -PassThru

  $baseUrl = "http://127.0.0.1:$Port"
  $url = "$baseUrl/"
  $ready = $false
  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    Start-Sleep -Milliseconds 300
    try {
      Invoke-SmokeRequest -Url $url -ExpectedStatus 200 -Expect "Axonyx" | Out-Null
      $ready = $true
      break
    } catch {
      if ($serverProcess.HasExited) {
        throw "Production server exited early. stdout: $(Get-Content -LiteralPath $stdout -Raw) stderr: $(Get-Content -LiteralPath $stderr -Raw)"
      }
    }
  }

  if (!$ready) {
    throw "Production server did not respond at $url"
  }

  Invoke-TemplateRouteChecks -BaseUrl $baseUrl -Template $Template
  Invoke-SmokeRequest -Url "$baseUrl/" -ExpectedStatus 200 -ExpectHeader "X-Content-Type-Options" -ExpectHeaderValue "nosniff" | Out-Null
  Invoke-SmokeCurlRequest -Url "$baseUrl/" -Headers @("Accept-Encoding: gzip") -ExpectedStatus 200 -Expect "Content-Encoding: gzip" | Out-Null
  Invoke-SmokeRequest -Url "$baseUrl/__axonyx/health" -ExpectedStatus 200 -Expect '"ok":true|"ok": true' -ExpectHeader "Content-Type" -ExpectHeaderValue "application/json" | Out-Null
  Invoke-OversizedHeaderSmoke -HostName "127.0.0.1" -Port $Port
  if ($Template -ne "docs") {
    Invoke-ChunkedPostSmoke -HostName "127.0.0.1" -Port $Port
  }
  Invoke-MalformedRequestSmoke -HostName "127.0.0.1" -Port $Port
  Invoke-SmokeRequest -Url "$baseUrl/favicon.svg" -ExpectedStatus 200 -ExpectHeader "Content-Type" -ExpectHeaderValue "image/svg\+xml" | Out-Null
  Invoke-SmokeRequest -Url "$baseUrl/favicon.svg" -ExpectedStatus 200 -ExpectHeader "Cache-Control" -ExpectHeaderValue "public, max-age=31536000, immutable" | Out-Null
  if ($Template -ne "minimal") {
    Invoke-SmokeRequest -Url "$baseUrl/_ax/pkg/axonyx-ui/index.css" -ExpectedStatus 200 -Expect "@import|--ax-" -ExpectHeader "Content-Type" -ExpectHeaderValue "text/css" | Out-Null
    Invoke-SmokeRequest -Url "$baseUrl/_ax/pkg/axonyx-ui/index.css" -ExpectedStatus 200 -ExpectHeader "Cache-Control" -ExpectHeaderValue "public, max-age=31536000, immutable" | Out-Null
  }
  Invoke-SmokeRequest -Url "$baseUrl/" -Method "HEAD" -ExpectedStatus 200 | Out-Null
  Invoke-SmokeRequest -Url "$baseUrl/definitely-missing" -ExpectedStatus 404 -Expect "not found|Not found|Page not found|Back to home" | Out-Null

  $serverLog = Get-Content -LiteralPath $stdout -Raw
  if ($serverLog -notmatch "using tokio transport") {
    throw "Expected server log to use tokio transport"
  }

  if ($serverLog -notmatch "Tokio graceful shutdown is enabled") {
    throw "Expected server log to report Tokio graceful shutdown"
  }

  if ($serverLog -notmatch "Shutdown grace period: 5 seconds") {
    throw "Expected server log to report shutdown grace period"
  }

  if ($serverLog -notmatch "Tokio max connections: 1024") {
    throw "Expected server log to report Tokio max connections"
  }

  if ($serverLog -notmatch "Request read timeout: 2 seconds") {
    throw "Expected server log to report request read timeout"
  }

  if ($serverLog -notmatch "Compression: enabled") {
    throw "Expected server log to report compression"
  }

  if ($serverLog -notmatch "Security headers: enabled") {
    throw "Expected server log to report security headers"
  }

  if ($serverLog -notmatch "Request logging: enabled \(text\) to stdout") {
    throw "Expected server log to report request logging"
  }

  if ($serverLog -notmatch "\[axonyx\] GET / 200") {
    throw "Expected server log to include request log lines"
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
