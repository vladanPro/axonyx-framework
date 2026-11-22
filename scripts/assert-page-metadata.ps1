function Assert-PageMetadata {
    param([string] $Url)
    foreach ($title in @('Fish & Chips', 'Another title')) {
        $escapedTitle = [Net.WebUtility]::HtmlEncode($title)
        $query = [uri]::EscapeDataString($title)
        $response = Invoke-WebRequest "${Url}?title=$query" -TimeoutSec 10
        $parts = $response.Content -split '</head>', 2
        if ($parts.Count -ne 2 -or
            [regex]::Matches($parts[0], '<title>').Count -ne 1 -or
            !$parts[0].Contains("<title>$escapedTitle | Axonyx</title>") -or
            $parts[0] -notmatch '<meta(?=[^>]*name="description")(?=[^>]*content="A quoted &quot;value&quot; &amp; plain text")' -or
            $parts[0] -notmatch '<link(?=[^>]*rel="canonical")(?=[^>]*href="https://example\.test/retry")' -or
            [regex]::Matches($response.Content, 'rel="canonical"').Count -ne 1 -or
            $response.Content.Contains('rel="alternate"')) {
            throw "Incorrect server-rendered metadata for title '$title' at $Url"
        }
    }
}
