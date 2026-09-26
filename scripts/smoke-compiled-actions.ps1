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
$originalSessionKey = $env:AX_SECRET_SESSION_KEY
$originalSessionCookieSecure = $env:AX_SECRET_SESSION_COOKIE_SECURE

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
    } elseif ($header.Key -ieq "Cookie") {
      $request.CookieContainer = New-Object System.Net.CookieContainer
      $request.CookieContainer.SetCookies([uri] $Url, [string] $header.Value)
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
  New-Item -ItemType Directory -Path (Join-Path $appRoot "app/login") -Force | Out-Null
  [System.IO.File]::WriteAllText((Join-Path $appRoot "app/login/page.asx"), @'
page Login() {
  return ASX {
    <form method="post" action="/__axonyx/action?path=%2Fposts&name=Logout">
      <input type="hidden" name="__ax_patch" value="1" />
      <button type="submit">Logout without JavaScript</button>
    </form>
  }
}
'@)
  $actionSource = [System.IO.File]::ReadAllText($actionsPath) + @'

action SetTheme(theme: string) {
  require input.theme in ["silver", "bronze", "gold"] else error("Theme is required.")
  cookie "theme" = input.theme
  patch draftStatus = input.theme
  revalidate "/posts"
  return ok()
}

action Noop() {
  return ok()
}

action UploadImage(image: File) -> FileRef {
  data saved = Storage.save("media", input.image)
  return json(saved)
}

action Logout() {
  Session.destroy()
  return ok()
}
'@
  [System.IO.File]::WriteAllText(
    $actionsPath,
    $actionSource,
    (New-Object System.Text.UTF8Encoding($false))
  )

  [System.IO.File]::AppendAllText(
    (Join-Path $appRoot "Axonyx.toml"),
    @'

[storage.media]
root = "storage/media"
access = "read-write"
max_file_bytes = "64kb"
'@,
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

  $apiRoot = Join-Path $appRoot "routes/api"
  New-Item -ItemType Directory -Path $apiRoot -Force | Out-Null
  [System.IO.File]::WriteAllText(
    (Join-Path $apiRoot "account.ax"),
    @'
type User {
  id: String
  email: String
  role: String
}

type UserPermission {
  id: Int
  user_id: String
  permission: String
}

type Credential {
  user_id: String
  email: String
  password_hash: String
}

query resolveCredential(email: String) -> Credential? {
  return db.credentials.where({ email: input.email }).first()
}

route POST "/api/login" {
  input:
    email: String
    password: String
  before Login.throttle(input.email, 2, 60)
  data credential = resolveCredential(input.email)
  data verified = Password.verifyOptional(input.password, credential?.password_hash)
  require verified
  require credential
  Session.create(credential.user_id, {})
  return json("ok")
}

fn hasRole(user: User, role: String) -> Bool {
  return user.role == role
}

query resolveUser(subject: String) -> User? {
  return db.users.where({ id: input.subject }).first()
}

query resolvePermission(userId: String, permission: String) -> UserPermission? {
  return db.user_permissions.where({ user_id: input.userId, permission: input.permission }).first()
}

route GET "/api/account" -> User {
  require Auth.subject else redirect("/login")
  data user = resolveUser(Auth.subject)
  require user else notFound()
  return json(user)
}

route GET "/api/admin" -> User {
  require Auth.subject else redirect("/login")
  data user = resolveUser(Auth.subject)
  require user else notFound()
  data isAdmin = hasRole(user, "admin")
  require isAdmin else forbidden()
  return json(user)
}

route GET "/api/publish" -> User {
  require Auth.subject
  data user = resolveUser(Auth.subject)
  require user else notFound()
  data grant = resolvePermission(user.id, "articles.publish")
  require grant else forbidden()
  return json(user)
}

route POST "/api/password-probe" {
  data verified = Password.verify(request.form.password, request.form.hash)
  require verified
  return json("ok")
}
'@,
    (New-Object System.Text.UTF8Encoding($false))
  )

  $dbPath = Join-Path $appRoot "compiled-smoke.db"
  $python = Get-Command python -ErrorAction SilentlyContinue
  if ($null -eq $python) { $python = Get-Command python3 -ErrorAction Stop }
  $schema = "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, slug TEXT, title TEXT NOT NULL, excerpt TEXT NOT NULL, status TEXT NOT NULL); CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT NOT NULL, role TEXT NOT NULL); CREATE TABLE user_permissions (id INTEGER PRIMARY KEY, user_id TEXT NOT NULL, permission TEXT NOT NULL, UNIQUE(user_id, permission)); CREATE TABLE credentials (user_id TEXT NOT NULL REFERENCES users(id), email TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL);"
  $seed = "INSERT INTO posts (slug,title,excerpt,status) VALUES (?,?,?,?)"
  $userSeed = "INSERT INTO users (id,email,role) VALUES (?,?,?)"
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.executescript(sys.argv[2]);db.execute(sys.argv[3],sys.argv[4:8]);db.execute(sys.argv[8],sys.argv[9:12]);db.commit();db.close()' $dbPath $schema $seed "fresh-compiled-post" "Original detail title" "Parameterized loader detail" "published" $userSeed "user-42" "foundry@example.com" "member"
  if ($LASTEXITCODE -ne 0) { throw "failed to seed compiled smoke SQLite database" }

  $passwordHash = cargo run --manifest-path (Join-Path $frameworkRoot "Cargo.toml") -p cargo-axonyx --example password_fixture --quiet
  if ($LASTEXITCODE -ne 0) { throw "failed to hash fixture password" }
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute("insert into credentials (user_id,email,password_hash) values (?,?,?)", ("user-42", "foundry@example.com", sys.argv[2]));db.commit();db.close()' $dbPath ([string] $passwordHash)
  if ($LASTEXITCODE -ne 0) { throw "failed to seed fixture credential" }

  $env:AX_SECRET_DB_DIALECT = "sqlite"
  $env:AX_SECRET_DB_URL = $dbPath
  $env:AX_SECRET_SESSION_KEY = "compiled-smoke-session-secret-at-least-32-bytes"
  $env:AX_SECRET_SESSION_COOKIE_SECURE = "false"

  Push-Location $appRoot
  try {
    $configPath = Join-Path $appRoot "Axonyx.toml"
    $configSource = [System.IO.File]::ReadAllText($configPath).Replace('[server]', "[server]`npublic_origin = `"http://127.0.0.1:$Port`"")
    [System.IO.File]::WriteAllText($configPath, $configSource)
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

  # Test-only probe; real login must load the hash from server-owned storage.
  $badHash = Invoke-AxRequest -Url "$baseUrl/api/password-probe" -Body "password=example&hash=invalid-secret-hash" -ExpectedStatus 500
  if ($badHash.Body -match "invalid-secret-hash" -or $badHash.Body -match "password=example") {
    throw "Password verification failure exposed secret input"
  }

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

  $uploadUrl = "$baseUrl/__axonyx/action?path=%2Fposts&name=UploadImage"
  $clientHandler = New-Object System.Net.Http.HttpClientHandler
  $clientHandler.AllowAutoRedirect = $false
  $client = New-Object System.Net.Http.HttpClient($clientHandler)
  $multipart = New-Object System.Net.Http.MultipartFormDataContent
  try {
    $multipart.Add((New-Object System.Net.Http.StringContent("true")), "__ax_patch")
    $fileContent = New-Object System.Net.Http.ByteArrayContent(,[System.Text.Encoding]::UTF8.GetBytes("compiled storage"))
    $fileContent.Headers.ContentType = New-Object System.Net.Http.Headers.MediaTypeHeaderValue("text/plain")
    $multipart.Add($fileContent, "image", "compiled.txt")
    $uploadRequest = New-Object System.Net.Http.HttpRequestMessage([System.Net.Http.HttpMethod]::Post, $uploadUrl)
    $uploadRequest.Headers.Accept.ParseAdd("application/ax-patch+json")
    $uploadRequest.Content = $multipart
    $uploadResponse = $client.SendAsync($uploadRequest).GetAwaiter().GetResult()
    $uploadBody = $uploadResponse.Content.ReadAsStringAsync().GetAwaiter().GetResult()
    if ([int]$uploadResponse.StatusCode -ne 200) { throw "Compiled upload returned HTTP $([int]$uploadResponse.StatusCode): $uploadBody" }
    $uploadPayload = $uploadBody | ConvertFrom-Json
    if (!$uploadPayload.ok -or $uploadPayload.value.storage -ne "media" -or $uploadPayload.value.file_name -ne "compiled.txt") {
      throw "Compiled storage action response is invalid: $uploadBody"
    }
  } finally {
    if ($null -ne $uploadResponse) { $uploadResponse.Dispose() }
    if ($null -ne $uploadRequest) { $uploadRequest.Dispose() }
    $multipart.Dispose()
    $client.Dispose()
    $clientHandler.Dispose()
  }
  $storedObjects = @(Get-ChildItem (Join-Path $appRoot "storage/media/objects") -File -Recurse)
  if ($storedObjects.Count -ne 1) { throw "Compiled storage action did not persist exactly one object" }

  $createUrl = "$baseUrl/__axonyx/action?path=%2Fposts&name=CreatePost"
  $created = Invoke-AxRequest -Url $createUrl -Body "title=Fresh+compiled+post&excerpt=Rendered+without+reload&status=published&__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" }
  $createdPayload = $created.Body | ConvertFrom-Json
  if (!$createdPayload.ok -or $createdPayload.refreshes[0].name -ne "posts") { throw "Compiled create action did not invalidate posts: $($created.Body)" }

  $themeCookie = Invoke-AxRequest -Url $actionUrl -Body "theme=gold&__ax_patch=true" -Headers @{ Accept = "application/ax-patch+json" }
  if ($themeCookie.Headers["Set-Cookie"] -notmatch "theme=gold") { throw "Compiled action response did not emit its cookie" }

  $loginUrl = "$baseUrl/api/login"
  $nativeForm = Invoke-AxRequest -Url "$baseUrl/login" -Method "GET"
  if ($nativeForm.Body -notmatch 'name="__ax_csrf" value="axcsrf1\.[a-f0-9]{64}"' -or $nativeForm.Body.Contains('<!--axonyx:csrf-field-->') -or $nativeForm.Headers["Cache-Control"] -ne "no-store") { throw "Native form did not receive a private anonymous proof" }
  $builtForm = [System.IO.File]::ReadAllText((Join-Path $appRoot "dist/login/index.html"))
  if (!$builtForm.Contains('<!--axonyx:csrf-field-->') -or $builtForm.Contains('name="__ax_csrf"')) { throw "Build artifact must not contain a personalized CSRF proof" }
  Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -Headers @{ Origin = $baseUrl } -ExpectedStatus 403 | Out-Null
  $anonymousProof = Invoke-AxRequest -Url "$baseUrl/__axonyx/csrf" -Method "GET" -Headers @{ Origin = $baseUrl }
  $anonymousToken = ($anonymousProof.Body | ConvertFrom-Json).token
  $anonymousCookie = ([string] $anonymousProof.Headers["Set-Cookie"]).Split(';')[0]
  $anonymousHeaders = @{ Origin = $baseUrl; Cookie = $anonymousCookie; "X-Axonyx-CSRF" = $anonymousToken }
  Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -Headers @{ Origin = "https://127.0.0.1:$Port" } -ExpectedStatus 403 | Out-Null
  Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -Headers @{ Origin = "https://attacker.example"; "X-Forwarded-Host" = "attacker.example" } -ExpectedStatus 403 | Out-Null
  $crossSite = Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -Headers @{ Origin = "https://attacker.example" } -ExpectedStatus 403
  if ($crossSite.Headers["Set-Cookie"] -or $crossSite.Headers["Cache-Control"] -ne "no-store") {
    throw "Cross-site API login must not issue a cookie or be cached"
  }
  $wrongPassword = Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=wrong" -Headers $anonymousHeaders -ExpectedStatus 401
  $unknownUser = Invoke-AxRequest -Url $loginUrl -Body "email=unknown%40example.com&password=wrong" -Headers $anonymousHeaders -ExpectedStatus 401
  if ($wrongPassword.Body -ne $unknownUser.Body -or $wrongPassword.Headers["Set-Cookie"] -or $unknownUser.Headers["Set-Cookie"]) {
    throw "Failed login must use a generic response and must not create a session"
  }
  $failedSessions = & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);exists=db.execute("select 1 from sqlite_master where type = ? and name = ?", ("table", "ax_sessions")).fetchone();print(db.execute("select count(*) from ax_sessions").fetchone()[0] if exists else 0);db.close()' $dbPath
  if ($LASTEXITCODE -ne 0 -or [int] $failedSessions -ne 0) { throw "Failed login persisted a session" }
  $login = Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -Headers $anonymousHeaders
  $sessionCookieHeader = [string] $login.Headers["Set-Cookie"]
  if ($sessionCookieHeader -notmatch "HttpOnly" -or $sessionCookieHeader -notmatch "session=") {
    throw "Compiled login did not emit a private session cookie: $sessionCookieHeader"
  }
  if ($login.Body -match "session" -or $login.Body -match "user-42" -or $login.Body.Contains([string] $passwordHash) -or $login.Body -match "compiled-smoke-password") {
    throw "Compiled login leaked session data into the action response"
  }
  $sessionCookie = $sessionCookieHeader.Split(';')[0]
  $tokenResponse = Invoke-AxRequest -Url "$baseUrl/__axonyx/csrf" -Method "GET" -Headers @{ Cookie = $sessionCookie; Origin = $baseUrl }
  $csrfToken = ($tokenResponse.Body | ConvertFrom-Json).token
  if ($csrfToken -notmatch '^axcsrf1\.[a-f0-9]{64}$' -or $tokenResponse.Headers["Cache-Control"] -ne "no-store" -or $tokenResponse.Headers["Vary"] -ne "Cookie") { throw "Invalid private CSRF token response" }
  if ($tokenResponse.Body -match "user-42" -or $tokenResponse.Body.Contains($sessionCookie.Split('=')[1])) { throw "CSRF response leaked session identity" }
  Invoke-AxRequest -Url "$baseUrl/__axonyx/csrf" -Method "GET" -Headers @{ Cookie = $sessionCookie; Origin = "https://attacker.example" } -ExpectedStatus 403 | Out-Null
  Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -Headers @{ Cookie = $sessionCookie; Origin = $baseUrl } -ExpectedStatus 403 | Out-Null
  $limited = Invoke-AxRequest -Url $loginUrl -Body "email=foundry%40example.com&password=compiled-smoke-password" -ExpectedStatus 429
  $retrySeconds = 0
  if (![int]::TryParse([string] $limited.Headers["Retry-After"], [ref] $retrySeconds) -or $retrySeconds -lt 1 -or $retrySeconds -gt 60) {
    throw "Login throttle must return a bounded Retry-After"
  }
  if ($limited.Headers["Set-Cookie"] -or $limited.Headers["Cache-Control"] -ne "no-store") {
    throw "Limited login must not issue cookies or be cached"
  }
  $activeSessions = & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);print(db.execute("select count(*) from ax_sessions").fetchone()[0]);db.close()' $dbPath
  if ($LASTEXITCODE -ne 0 -or [int] $activeSessions -ne 1) { throw "Limited login created another session" }
  $anonymousPublish = Invoke-AxRequest -Url "$baseUrl/api/publish" -Method "GET" -ExpectedStatus 401
  if (($anonymousPublish.Body | ConvertFrom-Json).error -ne "unauthorized") {
    throw "Permission route did not reject an anonymous request"
  }
  $account = Invoke-AxRequest -Url "$baseUrl/api/account" -Method "GET" -Headers @{ Cookie = $sessionCookie }
  $accountPayload = $account.Body | ConvertFrom-Json
  if ($accountPayload.id -ne "user-42" -or $accountPayload.email -ne "foundry@example.com") {
    throw "Compiled protected route did not resolve the typed Auth user: $($account.Body)"
  }
  $admin = Invoke-AxRequest -Url "$baseUrl/api/admin" -Method "GET" -Headers @{ Cookie = $sessionCookie } -ExpectedStatus 403
  $adminPayload = $admin.Body | ConvertFrom-Json
  if ($adminPayload.error -ne "forbidden") {
    throw "Compiled policy route did not return a safe forbidden response: $($admin.Body)"
  }
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute("insert into user_permissions (user_id, permission) values (?, ?)", ("other-user", "articles.publish"));db.commit();db.close()' $dbPath
  if ($LASTEXITCODE -ne 0) { throw "failed to seed another user's permission" }
  $deniedPublish = Invoke-AxRequest -Url "$baseUrl/api/publish" -Method "GET" -Headers @{ Cookie = $sessionCookie } -ExpectedStatus 403
  if (($deniedPublish.Body | ConvertFrom-Json).error -ne "forbidden") {
    throw "Permission route accepted another user's grant"
  }
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute("insert into user_permissions (user_id, permission) values (?, ?)", ("user-42", "articles.publish"));db.commit();db.close()' $dbPath
  if ($LASTEXITCODE -ne 0) { throw "failed to grant compiled smoke permission" }
  $authorizedPublish = Invoke-AxRequest -Url "$baseUrl/api/publish" -Method "GET" -Headers @{ Cookie = $sessionCookie }
  if (($authorizedPublish.Body | ConvertFrom-Json).id -ne "user-42") {
    throw "Permission route did not allow a persisted grant"
  }
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute("delete from user_permissions where user_id = ? and permission = ?", ("user-42", "articles.publish"));db.commit();db.close()' $dbPath
  if ($LASTEXITCODE -ne 0) { throw "failed to revoke compiled smoke permission" }
  Invoke-AxRequest -Url "$baseUrl/api/publish" -Method "GET" -Headers @{ Cookie = $sessionCookie } -ExpectedStatus 403 | Out-Null
  & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute("update users set role = ? where id = ?", ("admin", "user-42"));db.commit();db.close()' $dbPath
  if ($LASTEXITCODE -ne 0) { throw "failed to promote compiled smoke user" }
  $authorizedAdmin = Invoke-AxRequest -Url "$baseUrl/api/admin" -Method "GET" -Headers @{ Cookie = $sessionCookie }
  $authorizedAdminPayload = $authorizedAdmin.Body | ConvertFrom-Json
  if ($authorizedAdminPayload.id -ne "user-42" -or $authorizedAdminPayload.role -ne "admin") {
    throw "Compiled policy route did not return its narrowed typed user: $($authorizedAdmin.Body)"
  }
  $logoutUrl = "$baseUrl/__axonyx/action?path=%2Fposts&name=Logout"
  Invoke-AxRequest -Url $logoutUrl -Body "" -Headers @{ Cookie = $sessionCookie } -ExpectedStatus 403 | Out-Null
  Invoke-AxRequest -Url $logoutUrl -Body "" -Headers @{ Cookie = $sessionCookie; Origin = $baseUrl } -ExpectedStatus 403 | Out-Null
  Invoke-AxRequest -Url $logoutUrl -Body "" -Headers @{ Cookie = $sessionCookie; Origin = $baseUrl; "X-Axonyx-CSRF" = "invalid" } -ExpectedStatus 403 | Out-Null
  $stillAuthenticated = Invoke-AxRequest -Url "$baseUrl/api/account" -Method "GET" -Headers @{ Cookie = $sessionCookie }
  if (($stillAuthenticated.Body | ConvertFrom-Json).id -ne "user-42") { throw "Rejected logout changed the session" }
  $sessionForm = Invoke-AxRequest -Url "$baseUrl/login" -Method "GET" -Headers @{ Cookie = $sessionCookie }
  if ($sessionForm.Body -notmatch 'name="__ax_csrf" value="(axcsrf1\.[a-f0-9]{64})"') { throw "Native form did not receive a session proof" }
  $nativeToken = $Matches[1]
  $logout = Invoke-AxRequest -Url $logoutUrl -Body "__ax_csrf=$nativeToken&__ax_patch=1" -Headers @{ Accept = "text/html"; Cookie = $sessionCookie; Origin = $baseUrl; "X-Forwarded-Host" = "ignored.invalid" } -ExpectedStatus 303
  if ($logout.Headers["Set-Cookie"] -notmatch "Max-Age=0") {
    throw "Compiled logout did not clear the session cookie"
  }

  $sessionCount = & $python.Source -c 'import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);print(db.execute("select count(*) from ax_sessions").fetchone()[0]);db.close()' $dbPath
  if ($LASTEXITCODE -ne 0 -or [int] $sessionCount -ne 0) {
    throw "Compiled logout did not remove the persisted session"
  }
  Invoke-AxRequest -Url "$baseUrl/api/account" -Method "GET" -Headers @{ Cookie = $sessionCookie } -ExpectedStatus 303 | Out-Null
  $expiredToken = Invoke-AxRequest -Url "$baseUrl/__axonyx/csrf" -Method "GET" -Headers @{ Cookie = $sessionCookie; Origin = $baseUrl }
  if (($expiredToken.Body | ConvertFrom-Json).token -eq $csrfToken) { throw "Revoked session still issued its previous CSRF proof" }

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
  if ($null -eq $originalSessionKey) {
    Remove-Item Env:AX_SECRET_SESSION_KEY -ErrorAction SilentlyContinue
  } else {
    $env:AX_SECRET_SESSION_KEY = $originalSessionKey
  }
  if ($null -eq $originalSessionCookieSecure) {
    Remove-Item Env:AX_SECRET_SESSION_COOKIE_SECURE -ErrorAction SilentlyContinue
  } else {
    $env:AX_SECRET_SESSION_COOKIE_SECURE = $originalSessionCookieSecure
  }
  if ($ownsWorkDir -and (Test-Path -LiteralPath $WorkDir)) {
    $resolved = [System.IO.Path]::GetFullPath($WorkDir)
    $temp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if ($resolved.StartsWith($temp)) { Remove-Item -LiteralPath $resolved -Recurse -Force }
  }
}
