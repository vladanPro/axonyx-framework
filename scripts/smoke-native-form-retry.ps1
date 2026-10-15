param([int] $Port = 3929)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('axonyx-form-retry-' + [guid]::NewGuid().ToString('N'))
$process = $null
$environment = @{}
foreach ($item in @(Get-ChildItem Env: | Where-Object Name -Match '^(AX_SECRET_|AX_PUBLIC_|AXONYX_HOST$|AXONYX_PORT$)')) {
    $environment[$item.Name] = $item.Value
    [Environment]::SetEnvironmentVariable($item.Name, $null)
}

function Assert-Replay {
    param([string] $Base)
    $document = '{"draft":"draft-marker </textarea><script>literal</script>"}' + [char]::ConvertFromUtf32(0x1F642)
    $body = 'title=&content_document=' + [Net.WebUtility]::UrlEncode($document) + '&ignored=do-not-retain'
    $url = "$Base/__axonyx/action?path=%2F&name=SaveDocument"
    $response = Invoke-WebRequest $url -Method Post -Body $body -ContentType 'application/x-www-form-urlencoded' -Headers @{ Accept = 'text/html'; Origin = $Base } -SkipHttpErrorCheck -TimeoutSec 10
    if ($response.StatusCode -ne 422 -or $response.Content -notmatch 'Enter a title\.') { throw 'Expected native HTML validation response' }
    if (($response.Headers['Cache-Control'] -join ',') -notmatch '\bno-store\b') { throw 'Native retry response must not be cached' }
    $textareas = [regex]::Matches($response.Content, '(?s)<textarea\b([^>]*)>(.*?)</textarea>')
    $documentFields = @($textareas | Where-Object { $_.Groups[1].Value -match '\bname="content_document"' })
    if ($documentFields.Count -ne 1) { throw 'Expected one document textarea' }
    $field = $documentFields[0]
    if ($field.Groups[1].Value -notmatch '\bdisabled(?:=|\s|$)' -or $field.Groups[1].Value -notmatch 'data-ax-retained="true"') { throw 'Replay lost disabled state or recovery marker' }
    if ([Net.WebUtility]::HtmlDecode($field.Groups[2].Value) -cne $document -or $field.Groups[2].Value.Contains('<script>')) { throw 'Retained text was changed or emitted as markup' }
    $ignored = @($textareas | Where-Object { $_.Groups[1].Value -match '\bname="ignored"' })
    if ($ignored.Count -ne 1 -or $ignored[0].Groups[2].Value -cne 'original' -or $ignored[0].Groups[1].Value -match 'data-ax-retained=') { throw 'Unmarked disabled textarea was replayed' }
    $json = Invoke-WebRequest $url -Method Post -Body $body -ContentType 'application/x-www-form-urlencoded' -Headers @{ Accept = 'application/json'; Origin = $Base } -SkipHttpErrorCheck -TimeoutSec 10
    if ($json.StatusCode -ne 422 -or $json.Content.Contains('draft-marker')) { throw 'Private disabled document leaked into JSON response' }
    if (($json.Headers['Cache-Control'] -join ',') -notmatch '\bno-store\b') { throw 'JSON validation response must not be cached' }
}

try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    Copy-Item -Path (Join-Path $root 'tests/fixtures/native-form-retry/*') -Destination $fixture -Recurse
    $runtime = (Join-Path $root 'vendor/axonyx-runtime/crates/axonyx-runtime').Replace('\', '/')
    [IO.File]::WriteAllText((Join-Path $fixture 'Cargo.toml'), [IO.File]::ReadAllText((Join-Path $fixture 'Cargo.toml.tpl')).Replace('{{RUNTIME_PATH}}', $runtime))
    Push-Location $root
    try {
        & cargo build -p cargo-axonyx --bin cargo-axonyx
        if ($LASTEXITCODE -ne 0) { throw 'CLI build failed' }
        $metadata = & cargo metadata --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
        $target = ($metadata | ConvertFrom-Json).target_directory
    } finally { Pop-Location }
    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $cli = Join-Path $target "debug/cargo-axonyx$suffix"
    Push-Location $fixture
    try {
        & $cli build --clean --compiled
        if ($LASTEXITCODE -ne 0) { throw 'Compiled form fixture build failed' }
        foreach ($mode in @('preview', 'compiled')) {
            $arguments = if ($mode -eq 'preview') { @('run', 'dev', '--host', '127.0.0.1', '--port', "$Port") } else { @('run', 'start', '--compiled', '--host', '127.0.0.1', '--port', "$Port") }
            $options = @{
                FilePath = $cli
                ArgumentList = $arguments
                WorkingDirectory = $fixture
                PassThru = $true
                RedirectStandardOutput = (Join-Path $fixture "$mode.out.log")
                RedirectStandardError = (Join-Path $fixture "$mode.err.log")
            }
            if ($IsWindows) { $options.WindowStyle = 'Hidden' }
            $process = Start-Process @options
            $base = "http://127.0.0.1:$Port"
            $ready = $false
            for ($i = 0; $i -lt 100; $i++) {
                if ($process.HasExited) { throw "Fixture server exited: $([IO.File]::ReadAllText((Join-Path $fixture "$mode.err.log")))" }
                try {
                    $response = Invoke-WebRequest $base -SkipHttpErrorCheck -TimeoutSec 2
                    if ($response.StatusCode -eq 200 -and $response.Content.Contains('Native form retry smoke')) { $ready = $true; break }
                } catch { }
                Start-Sleep -Milliseconds 100
            }
            if (!$ready) { throw "$mode server did not become ready" }
            Assert-Replay -Base $base
            Write-Host "Native form retry passed ($mode): escaped document, disabled marker, unmarked exclusion and JSON privacy."
            $process.Kill($true)
            $process.WaitForExit()
            $process.Dispose()
            $process = $null
        }
    } finally { Pop-Location }
} finally {
    if ($null -ne $process) {
        if (!$process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
    foreach ($key in $environment.Keys) { [Environment]::SetEnvironmentVariable($key, $environment[$key]) }
    $resolved = [IO.Path]::GetFullPath($fixture)
    $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (!$resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notmatch '^axonyx-form-retry-[a-f0-9]{32}$') { throw 'Refusing cleanup outside generated form fixture' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
$global:LASTEXITCODE = 0
