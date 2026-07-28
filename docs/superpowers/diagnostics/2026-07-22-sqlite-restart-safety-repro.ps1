[CmdletBinding()]
param(
    [string]$CodexLogRoot = "$env:LOCALAPPDATA\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\Codex\Logs",
    [string]$AssistantThemeRoot = "$env:APPDATA\codex-agent-monitor\themes"
)

$ErrorActionPreference = 'Stop'

$sessionPath = Join-Path $AssistantThemeRoot 'control-session.json'
if (-not (Test-Path -LiteralPath $sessionPath)) {
    throw "No historical Codex Assistant control session was found at $sessionPath"
}

$sessionFile = Get-Item -LiteralPath $sessionPath
$session = Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
$pidToken = "-$($session.verified_pid)-t0-"
$candidateLogs = Get-ChildItem -LiteralPath $CodexLogRoot -Recurse -File -Filter '*.log' |
    Where-Object { $_.Name.Contains($pidToken) } |
    Sort-Object LastWriteTimeUtc

if ($candidateLogs.Count -eq 0) {
    throw "No Codex desktop log matched verified PID $($session.verified_pid)"
}

$failure = $candidateLogs |
    ForEach-Object {
        Select-String -LiteralPath $_.FullName -SimpleMatch 'failed to initialize sqlite state runtime' |
            Select-Object -First 1
    } |
    Select-Object -First 1

if (-not $failure) {
    [pscustomobject]@{
        Status = 'PASS'
        VerifiedPid = [int]$session.verified_pid
        SessionWrittenUtc = $sessionFile.LastWriteTimeUtc.ToString('o')
        SqliteFailure = $false
        Reason = 'The verified session PID has no SQLite initialization failure in its desktop log.'
    } | ConvertTo-Json
    exit 0
}

$match = [regex]::Match($failure.Line, '^(?<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z)')
if (-not $match.Success) {
    throw 'The matching SQLite error had no parseable UTC timestamp.'
}

$failureUtc = [datetime]::Parse(
    $match.Groups['timestamp'].Value,
    [Globalization.CultureInfo]::InvariantCulture,
    [Globalization.DateTimeStyles]::AssumeUniversal -bor [Globalization.DateTimeStyles]::AdjustToUniversal
)
$deltaMs = [math]::Round(($sessionFile.LastWriteTimeUtc - $failureUtc).TotalMilliseconds)
$falsePositive = $deltaMs -ge 0 -and $deltaMs -le 5000

[pscustomobject]@{
    Status = if ($falsePositive) { 'FAIL' } else { 'INCONCLUSIVE' }
    VerifiedPid = [int]$session.verified_pid
    SessionWrittenUtc = $sessionFile.LastWriteTimeUtc.ToString('o')
    SqliteFailureUtc = $failureUtc.ToString('o')
    SessionAfterFailureMs = $deltaMs
    SqliteFailure = $true
    Reason = if ($falsePositive) {
        'Codex Assistant persisted a verified theme session after the same process had already reported fatal SQLite initialization failure.'
    } else {
        'A SQLite failure exists, but it was not close enough to the saved session timestamp to prove the false-success race.'
    }
} | ConvertTo-Json

if ($falsePositive) {
    exit 1
}
