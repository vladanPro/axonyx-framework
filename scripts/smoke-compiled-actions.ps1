param(
  [int] $Port = 3921,
  [string] $WorkDir = ""
)

$ErrorActionPreference = "Stop"

$frameworkRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$ownsWorkDir = [string]::IsNullOrWhiteSpace($WorkDir)
if ($ownsWorkDir) {
  $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ("axonyx-compiled-actions-" + [guid]::NewGuid().ToString("N"))
}

$WorkDir = [System.IO.Path]::GetFullPath($WorkDir)
$appName = "compiled-actions-smoke"
$appRoot = Join-Path $WorkDir $appName
$stdout = Join-Path $WorkDir "server.out.log"
$stderr = Join-Path $WorkDir "server.err.log"
$serverProcess = $null
$originalLocation = Get-Location
$originalDbDialect = $env:AX_SECRET_DB_DIALECT
$originalDbUrl = $env:AX_SECRET_DB_URL

function Invoke-AxRequest {
  param(
    [Parameter(Mandatory = $true)] [string] $Url,
    [string] $Body = "",
    [hashtable] $Headers = @{},
    [string] $Method = "POST",
    [int] $ExpectedStatus = 200
  )

  $request = [System.Net.HttpWebRequest]::Create($Url)
  $request.Method = $Method
  $request.AllowAutoRedirect = $false
  if ($Method -ne "GET" -and $Method -ne "HEAD") {
    $request.ContentType = "application/x-www-form-urlencoded"
  }
  foreach ($header in $Headers.GetEnumerator()) {
    if ($header.Key -ieq "Accept") {
      $request.Accept = [string] $header.Value
    } elseif ($header.Key -ieq "Origin") {
      $request.Headers["Origin"] = [string] $header.Value
    } else {
      $request.Headers[$header.Key] = [string] $header.Value
    }
  }

  if ($Method -ne "GET" -and $Method -ne "HEAD") {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Body)
    $request.ContentLength = $bytes.Length
    $stream = $request.GetRequestStream()
    try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
  }

  try {
    $response = $request.GetResponse()
  } catch [System.Net.WebException] {
    if ($null -eq $_.Exception.Response) { throw }
    $response = $_.Exception.Response
  }

  try {
    $status = [int] $response.StatusCode
    if ($status -ne $ExpectedStatus) {
      throw "Expected HTTP $ExpectedStatus from $Url, got $status"
    }
    $reader = New-Object System.IO.StreamReader($response.GetResponseStream())
    try { $text = $reader.ReadToEnd() } finally { $reader.Dispose() }
    return @{ Status = $status; Body = $text; Headers = $response.Headers }
  } finally {
    $response.Dispose()
  }
}

New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null

try {
  Push-Location $WorkDir
  try {
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p create-axonyx -- $appName --yes --template minimal --runtime-source path
    if ($LASTEXITCODE -ne 0) { throw "create-axonyx failed" }
  } finally {
    Pop-Location
  }

  $actionsPath = Join-Path $appRoot "app/posts/actions.ax"
  $actionSource = [System.IO.File]::ReadAllText($actionsPath) + @'

action SetTheme(theme: string) {
  require input.theme in ["silver", "bronze", "gold"] else error("Theme is required.")
  patch draftStatus = input.theme
  revalidate "/posts"
  return ok()
}

action Noop() {
  return ok()
}
'@
  [System.IO.File]::WriteAllText(
    $actionsPath,
    $actionSource,
    (New-Object System.Text.UTF8Encoding($false))
  )

  $detailRoot = Join-Path $appRoot "app/posts/[slug]"
  New-Item -ItemType Directory -Path $detailRoot -Force | Out-Null
  [System.IO.File]::WriteAllText(
    (Join-Path $detailRoot "loader.ax"),
    @'
query loadPost(slug: String) {
  data post = db.posts.first()
    where slug = input.slug
  return post
}
'@,
    (New-Object System.Text.UTF8Encoding($false))
  )
  [System.IO.File]::WriteAllText(
    (Join-Path $detailRoot "actions.ax"),
    @'
action RenamePost(slug: string, title: string) {
  db.posts.where({ slug: input.slug }).update({ title: input.title })
  return ok()
}
'@,
    (New-Object System.Text.UTF8Encoding($false))
  )
  [System.IO.File]::WriteAllText(
    (Join-Path $detailRoot "page.ax"),
    @'
page PostDetail() {
data posts = loadPost(params.slug)
return ASX {
  <Container max="xl">
    <Card title={posts.title}>
      <Copy>{posts.excerpt}</Copy>
    </Card>
  </Container>
}
}
'@,
    (New-Object System.Text.UTF8Encoding($false))
  )

  $dbPath = Join-Path $appRoot "compiled-smoke.db"
  $python = Get-Command python -ErrorAction SilentlyContinue
  if ($null -eq $python) { $python = Get-Command python3 -ErrorAction Stop }
  $schema = "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, slug TEXT, title TEXT NOT NULL, excerpt TEXT NOT NULL, status TEXT NOT NULL)"
  $seed = "INSERT INTO posts (slug,title,excerpt,status) VALUES (?,?,?,?)"
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.execute(sys.argv[3],sys.argv[4:8]);db.commit();db.close()' $dbPath $schema $seed "fresh-compiled-post" "Original detail title" "Parameterized loader detail" "published"
  if ($LASTEXITCODE -ne 0) { throw "failed to seed compiled smoke SQLite database" }

  Push-Location $appRoot
  try {
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --bin cargo-axonyx -- check
    if ($LASTEXITCODE -ne 0) { throw "cargo ax check failed" }
    cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --bin cargo-axonyx -- build --clean --compiled
    if ($LASTEXITCODE -ne 0) { throw "compiled build failed" }
  } finally {
    Pop-Location
  }

  $args = @(
    "run", "--manifest-path", (Join-Path $frameworkRoot "Cargo.toml"),
    "-p", "cargo-axonyx", "--bin", "cargo-axonyx", "--",
    "run", "start", "--compiled", "--host", "127.0.0.1", "--port", "$Port"
  )
  $processArgs = @{
    FilePath = "cargo"
    ArgumentList = $args
    WorkingDirectory = $appRoot
    RedirectStandardOutput = $stdout
    RedirectStandardError = $stderr
    PassThru = $true
  }
  $env:AX_SECRET_DB_DIALECT = "sqlite"
  $env:AX_SECRET_DB_URL = $dbPath
  if ($env:OS -eq "Windows_NT") { $processArgs.WindowStyle = "Hidden" }
  $serverProcess = Start-Process @processArgs

  $baseUrl = "http://127.0.0.1:$Port"
  $ready = $false
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    Start-Sleep -Milliseconds 250
    try {
      $health = [System.Net.WebRequest]::Create("$baseUrl/__axonyx/health").GetResponse()
      $health.Dispose()
      $ready = $true
      break
    } catch {
      if ($serverProcess.HasExited) {
        throw "Compiled server exited early. stdout: $(Get-Content $stdout -Raw) stderr: $(Get-Content $stderr -Raw)"
      }
    }
  }
  if (!$ready) { throw "Compiled server did not become ready" }

  $readiness = Invoke-AxRequest -Url "$baseUrl/__axonyx/ready" -Method "GET"
  $readinessPayload = $readiness.Body | ConvertFrom-Json
  if (!$readinessPayload.ok -or !$readinessPayload.database.required -or !$readinessPayload.database.ok -or $readinessPayload.database.driver -ne "sqlite") {
    throw "Compiled database readiness response is invalid: $($readiness.Body)"
  }

  $actionUrl = "$baseUrl/__axonyx/action?path=%2Fposts&name=SetTheme"
  $success = Invoke-AxRequest -Url $actionUrl -Body "theme=gold&__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" }
  if ($success.Headers["Content-Type"] -notmatch "application/ax-patch\+json") { throw "Missing patch content type" }
  $payload = $success.Body | ConvertFrom-Json
  if (!$payload.ok -or $payload.redirect -ne "/posts") { throw "Compiled success envelope is invalid" }
  if ($payload.patches[0].signal -ne "page:posts:draftStatus:1" -or $payload.patches[0].value -ne "gold") { throw "Compiled state patch is invalid: $($success.Body)" }
  if ($payload.invalidations[0].target -ne "/posts" -or $payload.invalidations[0].queryKey[0] -ne "posts") { throw "Compiled invalidation is invalid" }
  if ($payload.refreshes[0].name -ne "posts" -or $payload.refreshes[0].source -ne "loadPosts()") { throw "Compiled data refresh metadata is invalid: $($success.Body)" }

  $createUrl = "$baseUrl/__axonyx/action?path=%2Fposts&name=CreatePost"
  $created = Invoke-AxRequest -Url $createUrl -Body "title=Fresh+compiled+post&excerpt=Rendered+without+reload&status=published&__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" }
  $createdPayload = $created.Body | ConvertFrom-Json
  if (!$createdPayload.ok -or $createdPayload.refreshes[0].name -ne "posts") { throw "Compiled create action did not invalidate posts: $($created.Body)" }

  $data = Invoke-AxRequest -Url "$baseUrl/__axonyx/data?path=%2Fposts&name=posts" -Method "GET" -Headers @{ Accept = "application/ax-data+json" }
  if ($data.Headers["Content-Type"] -notmatch "application/ax-data\+json") { throw "Missing compiled data content type" }
  $dataPayload = $data.Body | ConvertFrom-Json
  $createdRows = @($dataPayload.value | Where-Object { $_.title -eq "Fresh compiled post" })
  if (!$dataPayload.ok -or $dataPayload.binding.name -ne "posts" -or $createdRows.Count -ne 1) { throw "Compiled loader response is invalid: $($data.Body)" }
  if ($dataPayload.html -notmatch 'data-ax-root="page"' -or $dataPayload.html -notmatch "Fresh compiled post") { throw "Compiled page HTML was not regenerated: $($data.Body)" }

  $detailActionUrl = "$baseUrl/__axonyx/action?path=%2Fposts%2Ffresh-compiled-post&name=RenamePost"
  $renamed = Invoke-AxRequest -Url $detailActionUrl -Body "slug=fresh-compiled-post&title=Fresh+parameterized+title&__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" }
  $renamedPayload = $renamed.Body | ConvertFrom-Json
  if (!$renamedPayload.ok -or $renamedPayload.refreshes[0].name -ne "posts" -or $renamedPayload.refreshes[0].source -ne "loadPost(params.slug)") { throw "Parameterized action refresh metadata is invalid: $($renamed.Body)" }

  $detailData = Invoke-AxRequest -Url "$baseUrl/__axonyx/data?path=%2Fposts%2Ffresh-compiled-post&name=posts" -Method "GET" -Headers @{ Accept = "application/ax-data+json" }
  $detailPayload = $detailData.Body | ConvertFrom-Json
  if (!$detailPayload.ok -or $detailPayload.value.title -ne "Fresh parameterized title") { throw "Parameterized loader response is invalid: $($detailData.Body)" }
  if ($detailPayload.html -notmatch 'data-ax-root="page"' -or $detailPayload.html -notmatch "Fresh parameterized title") { throw "Parameterized page HTML was not regenerated: $($detailData.Body)" }
  Invoke-AxRequest -Url "$baseUrl/__axonyx/data?path=%2F%2Fevil.example&name=posts" -Method "GET" -ExpectedStatus 400 | Out-Null

  $invalid = Invoke-AxRequest -Url $actionUrl -Body "theme=&__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" } -ExpectedStatus 422
  if ($invalid.Headers["Content-Type"] -notmatch "application/ax-error\+json") { throw "Missing action error content type" }
  $errorPayload = $invalid.Body | ConvertFrom-Json
  if ($errorPayload.ok -or $errorPayload.error.message -ne "Theme is required.") { throw "Compiled validation envelope is invalid" }

  Invoke-AxRequest -Url "$baseUrl/__axonyx/action?path=%2Fposts&name=Missing" -Body "__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" } -ExpectedStatus 404 | Out-Null
  Invoke-AxRequest -Url $actionUrl -Body "theme=gold" -Headers @{ Origin = "https://attacker.example" } -ExpectedStatus 403 | Out-Null

  $fallback = Invoke-AxRequest -Url $actionUrl -Body "theme=silver" -ExpectedStatus 303
  if ($fallback.Headers["Location"] -ne "/posts") { throw "Compiled no-JS redirect fallback is invalid" }
  $safeFallback = Invoke-AxRequest -Url "$baseUrl/__axonyx/action?path=%2F%2Fevil.example&name=Noop" -Body "noop=1" -ExpectedStatus 303
  if ($safeFallback.Headers["Location"] -ne "/") { throw "Compiled action allowed an unsafe redirect" }

  Write-Host "Axonyx compiled action smoke passed."
} finally {
  if ($null -ne $serverProcess -and !$serverProcess.HasExited) {
    Stop-Process -Id $serverProcess.Id
    $serverProcess.WaitForExit(5000) | Out-Null
  }
  Set-Location $originalLocation
  if ($null -eq $originalDbDialect) {
    Remove-Item Env:AX_SECRET_DB_DIALECT -ErrorAction SilentlyContinue
  } else {
    $env:AX_SECRET_DB_DIALECT = $originalDbDialect
  }
  if ($null -eq $originalDbUrl) {
    Remove-Item Env:AX_SECRET_DB_URL -ErrorAction SilentlyContinue
  } else {
    $env:AX_SECRET_DB_URL = $originalDbUrl
  }
  if ($ownsWorkDir -and (Test-Path -LiteralPath $WorkDir)) {
    $resolved = [System.IO.Path]::GetFullPath($WorkDir)
    $temp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if ($resolved.StartsWith($temp)) { Remove-Item -LiteralPath $resolved -Recurse -Force }
  }
}
