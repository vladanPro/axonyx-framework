function Assert-ReactiveTableHtml {
  param([Parameter(Mandatory = $true)] [string] $Html)

  if ($Html -notmatch '<tbody\b[^>]*data-ax-each-row-mode="tr"' -or
      $Html -notmatch '<tr\b[^>]*data-ax-each-empty' -or
      $Html -notmatch '<template\b[^>]*data-ax-each-render-protocol="ax-each-render/1"' -or
      !$Html.Contains("No matching posts") -or $Html.Contains("<ax-state-each")) {
    throw "Expected reactive native table ownership, empty row, and template in HTML"
  }
}

function Assert-ReactiveTableServed {
  param([string] $Base)
  $page = Invoke-WebRequest "$Base/reactive-table" -TimeoutSec 10
  Assert-ReactiveTableHtml -Html $page.Content
  $wasm = Invoke-WebRequest "$Base/_ax/runtime/axonyx-state-v2.wasm" -TimeoutSec 10
  $contentType = [string] $wasm.Headers["Content-Type"]
  $magic = [byte[]] (0, 97, 115, 109)
  $stream = $wasm.RawContentStream
  $stream.Position = 0
  $bytes = [byte[]]::new(4)
  if (!$contentType.StartsWith("application/wasm", [StringComparison]::OrdinalIgnoreCase) -or
      $stream.Read($bytes, 0, $bytes.Length) -ne $bytes.Length -or
      [Convert]::ToHexString($bytes) -ne [Convert]::ToHexString($magic)) {
    throw "Expected the reactive table WASM asset at $Base"
  }
}
