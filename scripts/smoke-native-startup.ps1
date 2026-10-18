param([int] $Port = 3927)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('axonyx-native-startup-' + [guid]::NewGuid().ToString('N'))
$process = $null
$originalHost = $env:AXONYX_HOST
$originalPort = $env:AXONYX_PORT
$originalFail = $env:AXONYX_TEST_NATIVE_FAIL
$originalStorageFail = $env:AXONYX_TEST_STORAGE_FAIL
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    Copy-Item -Path (Join-Path $root 'tests/fixtures/native-startup/*') -Destination $fixture -Recurse
    $runtime = (Join-Path $root 'vendor/axonyx-runtime/crates/axonyx-runtime').Replace('\', '/')
    (Get-Content -LiteralPath (Join-Path $fixture 'Cargo.toml.tpl') -Raw).Replace('{{RUNTIME_PATH}}', $runtime) |
        Set-Content -LiteralPath (Join-Path $fixture 'Cargo.toml') -Encoding utf8
    Push-Location $root
    try {
        $metadata = & cargo metadata --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
        $target = ($metadata | ConvertFrom-Json).target_directory
        & cargo build -p cargo-axonyx --bin cargo-axonyx
        if ($LASTEXITCODE -ne 0) { throw 'CLI build failed' }
    } finally { Pop-Location }
    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $cli = Join-Path $target "debug/cargo-axonyx$suffix"
    Push-Location $fixture
    $originalTarget = $env:CARGO_TARGET_DIR
    $env:CARGO_TARGET_DIR = Join-Path $target 'native-startup-proof'
    try {
        & $cli build --clean --compiled
        if ($LASTEXITCODE -ne 0) { throw 'Native compiled application build failed' }
        $appMetadata = & cargo metadata --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Application metadata failed' }
        $builtBinary = Join-Path (($appMetadata | ConvertFrom-Json).target_directory) "release/axonyx-production$suffix"
        $binary = Join-Path $fixture "native-startup-server$suffix"
        Copy-Item -LiteralPath $builtBinary -Destination $binary
        $preview = (& $cli run dev 2>&1 | Out-String)
        if ($LASTEXITCODE -eq 0 -or $preview -notmatch 'cannot execute application Rust callbacks') {
            throw "Preview guard failed (exit $LASTEXITCODE): $preview"
        }
    } finally { $env:CARGO_TARGET_DIR = $originalTarget; Pop-Location }
    $env:AXONYX_HOST = '127.0.0.1'
    $env:AXONYX_PORT = "$Port"
    $env:AXONYX_TEST_NATIVE_FAIL = '1'
    $args = @{
        FilePath = $binary
        WorkingDirectory = $fixture
        PassThru = $true
        RedirectStandardOutput = (Join-Path $fixture 'server.out.log')
        RedirectStandardError = (Join-Path $fixture 'server.err.log')
    }
    if ($IsWindows) { $args.WindowStyle = 'Hidden' }
    $process = Start-Process @args
    if (-not $process.WaitForExit(10000)) { throw 'Invalid registration left the server running' }
    $process.WaitForExit()
    $startup = Get-Content -LiteralPath (Join-Path $fixture 'server.out.log') -Raw
    if ($process.ExitCode -eq 0 -or $startup -match 'listening') { throw 'Invalid registration did not stop startup' }
    $env:AXONYX_TEST_NATIVE_FAIL = $null
    $env:AXONYX_TEST_STORAGE_FAIL = '1'
    $process = Start-Process @args
    if (-not $process.WaitForExit(10000)) { throw 'Invalid storage configuration left the server running' }
    $process.WaitForExit()
    $startup = Get-Content -LiteralPath (Join-Path $fixture 'server.out.log') -Raw
    if ($process.ExitCode -eq 0 -or $startup -match 'listening') { throw 'Invalid storage configuration did not stop startup' }
    $env:AXONYX_TEST_STORAGE_FAIL = $null
    $process = Start-Process @args
    $base = "http://127.0.0.1:$Port"
    $ready = $false
    for ($i = 0; $i -lt 100; $i++) {
        if ($process.HasExited) { throw "Native server exited: $(Get-Content (Join-Path $fixture 'server.err.log') -Raw)" }
        try {
            $response = Invoke-WebRequest "$base/api/native?t%65xt=%20hello%20" -SkipHttpErrorCheck
            if ($response.StatusCode -eq 200) { $ready = $true; break }
        } catch { }
        Start-Sleep -Milliseconds 100
    }
    if (-not $ready) { throw 'Native HTTP route did not become ready' }
    if (($response.Content | ConvertFrom-Json).text -ne 'hello') { throw 'Registered callback output mismatch' }
    foreach ($path in @('/api/native-invalid', '/api/native-unknown', '/api/native?text=reject')) {
        $response = Invoke-WebRequest "$base$path" -SkipHttpErrorCheck
        if ($response.StatusCode -ne 500) { throw "Expected safe HTTP 500 for $path" }
        if ($response.Content -match 'private callback diagnostic|content.missing') { throw 'Native error leaked private details' }
    }
    $client = [Net.Http.HttpClient]::new()
    try {
        function Send-Upload([string]$Action, [string]$Key, [string]$Bytes, [int]$Status, [string]$Origin = $base) {
            $body = [Net.Http.MultipartFormDataContent]::new()
            $body.Add([Net.Http.StringContent]::new($Key), 'key')
            $body.Add([Net.Http.ByteArrayContent]::new([Text.Encoding]::UTF8.GetBytes($Bytes)), 'image', 'untrusted.txt')
            $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Post, "$base/__axonyx/action?path=%2Fupload&name=$Action")
            $request.Content = $body
            $request.Headers.TryAddWithoutValidation('Origin', $Origin) | Out-Null
            $request.Headers.TryAddWithoutValidation('Accept', 'application/ax-patch+json') | Out-Null
            try {
                $response = $client.SendAsync($request).GetAwaiter().GetResult()
                try {
                    $text = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
                    if ([int]$response.StatusCode -ne $Status) { throw "Upload $Action expected $Status, got $($response.StatusCode): $text" }
                    return $text
                } finally { $response.Dispose() }
            } finally { $request.Dispose() }
        }
        Send-Upload UploadCustom wrong source 403 | Out-Null
        Send-Upload UploadCustom fixture-only source 403 'https://attacker.example' | Out-Null
        # Current parser exposes an over-budget multipart body as HTTP 400.
        Send-Upload UploadCustom fixture-only ('x' * 5000) 400 | Out-Null
        Send-Upload UploadMissing fixture-only source 500 | Out-Null
        foreach ($invalid in @('UploadCustom&name=UploadRaw', 'UploadCustom&%6Eame=UploadRaw', 'UploadCustom&path=%2Fother', 'UploadCustom&extra=%GG')) {
            Send-Upload $invalid fixture-only source 400 | Out-Null
        }
        if (Test-Path (Join-Path $fixture 'custom-saved')) { throw 'Rejected upload reached custom storage' }
        $saved = (Send-Upload 'Upload%43ustom' fixture-only source 200 | ConvertFrom-Json).value
        if ($saved.id -ne 'custom-id' -or $saved.file_name -ne 'canonical.bin' -or $saved.size -ne 9) { throw 'Custom FileRef metadata mismatch' }
        if ((Get-Content (Join-Path $fixture 'custom-saved') -Raw) -ne 'canonical') { throw 'Custom storage did not receive actual multipart bytes' }
        $raw = (Send-Upload UploadRaw fixture-only fallback 200 | ConvertFrom-Json).value
        if ($raw.storage -ne 'raw' -or $raw.size -ne 8) { throw 'Configured fallback storage was not preserved' }
        if (-not (Test-Path (Join-Path $fixture 'storage-configured'))) { throw 'Application storage hook was not called' }
    } finally { $client.Dispose() }
    Write-Host 'Native startup passed: callback/storage failure, preview guard, typed HTTP calls, guarded custom multipart writes, body bounds and configured fallback.'
} finally {
    if ($null -ne $process -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    $env:AXONYX_HOST = $originalHost
    $env:AXONYX_PORT = $originalPort
    $env:AXONYX_TEST_NATIVE_FAIL = $originalFail
    $env:AXONYX_TEST_STORAGE_FAIL = $originalStorageFail
    $resolved = [IO.Path]::GetFullPath($fixture)
    $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notmatch '^axonyx-native-startup-[a-f0-9]{32}$') {
        throw 'Refusing cleanup outside generated native fixture'
    }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
# The preview rejection is expected; do not propagate its exit code to CI.
$global:LASTEXITCODE = 0
