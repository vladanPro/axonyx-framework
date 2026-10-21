param([int]$Port = 3931)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('axonyx-private-binary-' + [guid]::NewGuid().ToString('N'))
$process = $null
$originalHost = $env:AXONYX_HOST
$originalPort = $env:AXONYX_PORT
$originalTarget = $env:CARGO_TARGET_DIR
$client = $null
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    Copy-Item -Path (Join-Path $root 'tests/fixtures/native-startup/*') -Destination $fixture -Recurse
    Copy-Item -LiteralPath (Join-Path $root 'tests/fixtures/private-binary/native.rs') -Destination (Join-Path $fixture 'src/native.rs')
    Copy-Item -LiteralPath (Join-Path $root 'tests/fixtures/private-binary/private.ax') -Destination (Join-Path $fixture 'routes/api/private.ax')
    $runtime = (Join-Path $root 'vendor/axonyx-runtime/crates/axonyx-runtime').Replace('\', '/')
    (Get-Content -LiteralPath (Join-Path $fixture 'Cargo.toml.tpl') -Raw).Replace('{{RUNTIME_PATH}}', $runtime) |
        Set-Content -LiteralPath (Join-Path $fixture 'Cargo.toml') -Encoding utf8
    $configPath = Join-Path $fixture 'Axonyx.toml'
    $config = [IO.File]::ReadAllText($configPath).Replace('[server]', "[server]`napi_response_validation = `"always`"")
    [IO.File]::WriteAllText($configPath, $config)
    $db = (Join-Path $fixture 'sessions.sqlite').Replace('\', '/')
    Set-Content -LiteralPath (Join-Path $fixture '.env') -Value "AX_SECRET_DB_DRIVER=sqlite`nAX_SECRET_DB_URL=$db`nAX_SECRET_SESSION_KEY=isolated-private-binary-fixture-secret`nAX_SECRET_SESSION_COOKIE_SECURE=false"
    Push-Location $root
    try {
        $metadata = & cargo metadata --no-deps --format-version 1 --locked
        if ($LASTEXITCODE -ne 0) { throw 'Cannot locate isolated build cache' }
        $target = ($metadata | ConvertFrom-Json).target_directory
        & cargo build -p cargo-axonyx --bin cargo-axonyx --locked
        if ($LASTEXITCODE -ne 0) { throw 'CLI build failed' }
    } finally { Pop-Location }
    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $cli = Join-Path $target "debug/cargo-axonyx$suffix"
    Push-Location $fixture
    try {
        & $cli check
        if ($LASTEXITCODE -ne 0) { throw 'Binary route check failed' }
        & $cli api --openapi --out public/openapi.json
        if ($LASTEXITCODE -ne 0) { throw 'Binary OpenAPI export failed' }
        $contract = Get-Content 'public/openapi.json' -Raw | ConvertFrom-Json
        if ($contract.paths.'/api/private/{id}'.get.responses.'200'.content.'application/octet-stream'.schema.format -ne 'binary') { throw 'Binary OpenAPI contract mismatch' }
        if ($contract.paths.'/api/public/{id}'.get.security) { throw 'Public binary OpenAPI incorrectly requires a session' }
        if ($contract.paths.'/api/public/{id}'.get.responses.'200'.content.'image/png'.schema.format -ne 'binary') { throw 'Public Binary OpenAPI mismatch' }
        $env:CARGO_TARGET_DIR = Join-Path $target 'native-startup-proof'
        & $cli build --clean --compiled
        if ($LASTEXITCODE -ne 0) { throw 'Compiled Binary application build failed' }
        $appMetadata = & cargo metadata --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Application metadata failed' }
        $builtBinary = Join-Path (($appMetadata | ConvertFrom-Json).target_directory) "release/axonyx-production$suffix"
        $binary = Join-Path $fixture "private-binary-server$suffix"
        Copy-Item -LiteralPath $builtBinary -Destination $binary
    } finally { $env:CARGO_TARGET_DIR = $originalTarget; Pop-Location }
    $env:AXONYX_HOST = '127.0.0.1'
    $env:AXONYX_PORT = "$Port"
    $args = @{
        FilePath = $binary; WorkingDirectory = $fixture; PassThru = $true
        RedirectStandardOutput = (Join-Path $fixture 'server.out.log')
        RedirectStandardError = (Join-Path $fixture 'server.err.log')
    }
    if ($IsWindows) { $args.WindowStyle = 'Hidden' }
    $process = Start-Process @args
    $base = "http://127.0.0.1:$Port"
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [Net.Http.HttpClient]::new($handler)
    $client.DefaultRequestHeaders.Add('Origin', $base)
    $ready = $false
    for ($i = 0; $i -lt 100; $i++) {
        if ($process.HasExited) { throw "Server exited: $(Get-Content (Join-Path $fixture 'server.err.log') -Raw)" }
        try {
            $reply = $client.GetAsync("$base/api/private/image").GetAwaiter().GetResult()
            $ready = $true
            break
        } catch { Start-Sleep -Milliseconds 100 }
    }
    if (-not $ready) { throw 'Private binary server did not become ready' }
    if ([int]$reply.StatusCode -ne 403) { throw 'Anonymous binary request was not denied' }
    $reply.Dispose()
    $client.DefaultRequestHeaders.Add('If-None-Match', '*')
    foreach ($case in @(@('public/image', 200), @('public/missing', 404), @('public/reject', 500), @('public-private-channel', 500), @('public-unknown', 500))) {
        $reply = $client.GetAsync("$base/api/$($case[0])").GetAwaiter().GetResult()
        try {
            if ([int]$reply.StatusCode -ne $case[1]) { throw "Unexpected anonymous public status for $($case[0])" }
            if (-not $reply.Headers.CacheControl.NoStore) { throw 'Public response was cacheable' }
            if (($reply.Headers.GetValues('X-Content-Type-Options') -join '') -ne 'nosniff') { throw 'Public nosniff missing' }
            if ($reply.Headers.Contains('Set-Cookie')) { throw 'Public response created a session' }
            $bytes = $reply.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
            if ($case[1] -eq 200) {
                if (($bytes -join ',') -ne '0,255,128,42' -or $reply.Content.Headers.ContentType.MediaType -ne 'image/png') { throw 'Public binary bytes/MIME changed' }
            } elseif ($case[1] -eq 500 -and [Text.Encoding]::UTF8.GetString($bytes) -ne 'binary response unavailable') { throw 'Public failure leaked diagnostics' }
        } finally { $reply.Dispose() }
    }
    $null = $client.DefaultRequestHeaders.Remove('If-None-Match')
    $client.DefaultRequestHeaders.Add('X-Fixture', 'yes')
    $reply = $client.GetAsync("$base/api/guard/allowed?mode=compiled&public=yes").GetAwaiter().GetResult()
    try {
        if ([int]$reply.StatusCode -ne 403) { throw 'Anonymous compound subject guard retained access' }
    } finally { $reply.Dispose(); $null = $client.DefaultRequestHeaders.Remove('X-Fixture') }
    $reply = $client.GetAsync("$base/__axonyx/csrf").GetAwaiter().GetResult()
    if ([int]$reply.StatusCode -ne 200) { throw 'Pre-login CSRF token request failed' }
    $csrf = ($reply.Content.ReadAsStringAsync().Result | ConvertFrom-Json).token
    $reply.Dispose()
    if ([string]::IsNullOrEmpty($csrf)) { throw 'Pre-login CSRF token missing' }
    $client.DefaultRequestHeaders.Add('X-Axonyx-CSRF', $csrf)
    $empty = [Net.Http.StringContent]::new('')
    try {
        $reply = $client.PostAsync("$base/api/fixture-login?key=wrong", $empty).GetAwaiter().GetResult()
        try {
            if ([int]$reply.StatusCode -ne 403) { throw 'Incorrect inline query guard was not denied' }
            if ($reply.Headers.Contains('Set-Cookie')) { throw 'Rejected fixture login created a session' }
        } finally { $reply.Dispose() }
        $reply = $client.PostAsync("$base/api/fixture-login?key=isolated-fixture-only", $empty).GetAwaiter().GetResult()
        if ([int]$reply.StatusCode -ne 200) { throw "Fixture session failed: $($reply.Content.ReadAsStringAsync().Result)" }
        $reply.Dispose()
        $sessionCookie = $handler.CookieContainer.GetCookieHeader([uri]$base)
        if ([string]::IsNullOrEmpty($sessionCookie)) { throw 'Fixture login did not create a session' }
        $client.DefaultRequestHeaders.Add('X-Fixture', 'yes')
        foreach ($case in @(
            @('allowed?mode=compiled&public=yes', 200),
            @('allowed?mode=preview&public=no&key=isolated-fixture-only', 200),
            @('allowed?mode=unknown&public=yes', 403),
            @('denied?mode=compiled&public=yes', 403),
            @('allowed?mode=compiled&public=no&key=wrong', 403)
        )) {
            $reply = $client.GetAsync("$base/api/guard/$($case[0])").GetAwaiter().GetResult()
            try {
                if ([int]$reply.StatusCode -ne $case[1]) { throw "Unexpected compound guard status for $($case[0])" }
                if ($case[1] -eq 200) {
                    $value = $reply.Content.ReadAsStringAsync().Result | ConvertFrom-Json
                    if ($value.subject -ne 'fixture-owner' -or $value.literal -ne 'query.key params.id Auth.subject') { throw 'Compound guard altered session identity or literal text' }
                }
            } finally { $reply.Dispose() }
        }
        $null = $client.DefaultRequestHeaders.Remove('X-Fixture')
        $reply = $client.GetAsync("$base/api/guard/allowed?mode=compiled&public=yes").GetAwaiter().GetResult()
        try {
            if ([int]$reply.StatusCode -ne 403) { throw 'Missing request-header guard was not denied' }
        } finally { $reply.Dispose() }
        foreach ($case in @(@('image', 200), @('missing', 404), @('reject', 500))) {
            $reply = $client.GetAsync("$base/api/private/$($case[0])").GetAwaiter().GetResult()
            try {
                if ([int]$reply.StatusCode -ne $case[1]) { throw "Unexpected private status for $($case[0])" }
                if ($reply.Headers.CacheControl.ToString() -ne 'no-store, private' -and $reply.Headers.CacheControl.ToString() -ne 'private, no-store') { throw 'Missing private no-store policy' }
                if (($reply.Headers.GetValues('X-Content-Type-Options') -join '') -ne 'nosniff') { throw 'Missing nosniff policy' }
                if ($reply.Headers.Contains('Set-Cookie')) { throw 'Binary response changed session cookies' }
                if ($case[1] -eq 200) {
                    $bytes = $reply.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
                    if (($bytes -join ',') -ne '0,255,128,42') { throw 'Binary HTTP bytes were changed' }
                    if ($reply.Content.Headers.ContentType.MediaType -ne 'application/octet-stream') { throw 'Incorrect binary MIME' }
                }
            } finally { $reply.Dispose() }
        }
        $reply = $client.GetAsync("$base/api/private-unknown").GetAwaiter().GetResult()
        if ([int]$reply.StatusCode -ne 500 -or $reply.Content.ReadAsStringAsync().Result -ne 'binary response unavailable') { throw 'Unknown callback was not safely redacted' }
        $reply.Dispose()
        $reply = $client.GetAsync("$base/__axonyx/csrf").GetAwaiter().GetResult()
        if ([int]$reply.StatusCode -ne 200) { throw 'Fixture CSRF token request failed' }
        $csrf = ($reply.Content.ReadAsStringAsync().Result | ConvertFrom-Json).token
        $reply.Dispose()
        if ([string]::IsNullOrEmpty($csrf)) { throw 'Fixture CSRF token missing' }
        $null = $client.DefaultRequestHeaders.Remove('X-Axonyx-CSRF')
        $client.DefaultRequestHeaders.Add('X-Axonyx-CSRF', $csrf)
        $reply = $client.PostAsync("$base/api/fixture-unpublish", $empty).GetAwaiter().GetResult()
        try {
            if ([int]$reply.StatusCode -ne 200) { throw 'Fixture unpublish failed' }
        } finally { $reply.Dispose() }
        $client.DefaultRequestHeaders.Add('If-None-Match', '*')
        $reply = $client.GetAsync("$base/api/public/image").GetAwaiter().GetResult()
        try {
            if ([int]$reply.StatusCode -ne 404 -or -not $reply.Headers.CacheControl.NoStore) { throw 'Revoked public grant retained access' }
        } finally { $reply.Dispose(); $null = $client.DefaultRequestHeaders.Remove('If-None-Match') }
        $reply = $client.PostAsync("$base/api/fixture-logout", $empty).GetAwaiter().GetResult()
        if ([int]$reply.StatusCode -ne 200) { throw "Fixture logout failed ($([int]$reply.StatusCode)): $($reply.Content.ReadAsStringAsync().Result)" }
        $reply.Dispose()
        $client.DefaultRequestHeaders.Add('Cookie', $sessionCookie)
        $reply = $client.GetAsync("$base/api/private/image").GetAwaiter().GetResult()
        if ([int]$reply.StatusCode -ne 403) { throw 'Logged-out session retained binary access' }
        $reply.Dispose()
        $client.DefaultRequestHeaders.Add('X-Fixture', 'yes')
        $reply = $client.GetAsync("$base/api/guard/allowed?mode=compiled&public=yes").GetAwaiter().GetResult()
        try {
            if ([int]$reply.StatusCode -ne 403) { throw 'Logged-out compound subject guard retained access' }
        } finally { $reply.Dispose() }
    } finally { $empty.Dispose() }
    Write-Host 'Private/public Binary CLI proof passed: check, OpenAPI, compiled Axum HTTP, exact bytes, anonymous public admission, current grant revocation, private channel isolation/logout denial and safe errors with validation always.'
} finally {
    if ($null -ne $client) { $client.Dispose() }
    if ($null -ne $process -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    $env:AXONYX_HOST = $originalHost
    $env:AXONYX_PORT = $originalPort
    $env:CARGO_TARGET_DIR = $originalTarget
    $resolved = [IO.Path]::GetFullPath($fixture)
    $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notmatch '^axonyx-private-binary-[a-f0-9]{32}$') { throw 'Refusing cleanup outside generated private fixture' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
