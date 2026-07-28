param(
    [string]$ThemeRoot = (Join-Path $env:APPDATA 'codex-agent-monitor\themes')
)

$ErrorActionPreference = 'Stop'

$themeStatePath = Join-Path $ThemeRoot 'theme-state.json'
$sessionPath = Join-Path $ThemeRoot 'control-session.json'
$themeState = if (Test-Path -LiteralPath $themeStatePath -PathType Leaf) {
    Get-Content -LiteralPath $themeStatePath -Raw | ConvertFrom-Json
} else {
    [pscustomobject]@{ selected_theme_id = $null }
}
$session = if (Test-Path -LiteralPath $sessionPath -PathType Leaf) {
    Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
} else {
    $null
}

$codexRoots = @(
    Get-CimInstance Win32_Process |
        Where-Object {
            $_.Name -eq 'ChatGPT.exe' -and
            $_.ExecutablePath -match 'OpenAI\.Codex_' -and
            $_.CommandLine -notmatch '--type='
        }
)
$verifiedProcess = if ($null -ne $session) {
    Get-Process -Id ([int]$session.verified_pid) -ErrorAction SilentlyContinue
} else {
    $null
}
$listener = if ($null -ne $session) {
    Get-NetTCPConnection -State Listen -LocalPort ([int]$session.port) -ErrorAction SilentlyContinue
} else {
    $null
}
$ready = $codexRoots.Count -gt 0 -and $null -ne $verifiedProcess -and $null -ne $listener
$status = if ($codexRoots.Count -eq 0) {
    'codex-not-running'
} elseif ($ready) {
    'ready'
} else {
    'restart-required'
}

$result = [ordered]@{
    selected_theme_id = $themeState.selected_theme_id
    codex_root_pids = @($codexRoots.ProcessId)
    status = $status
    saved_verified_pid = if ($session) { [int]$session.verified_pid } else { $null }
    saved_port = if ($session) { [int]$session.port } else { $null }
    saved_codex_version = if ($session) { [string]$session.codex_version } else { $null }
    saved_pid_alive = $null -ne $verifiedProcess
    saved_port_listening = $null -ne $listener
    ready_for_one_click_theme = $ready
}

$result | ConvertTo-Json -Depth 3

if ($result.selected_theme_id -and $codexRoots.Count -gt 0 -and -not $result.ready_for_one_click_theme) {
    Write-Error 'THEME_APPLY_UNAVAILABLE: a theme is selected, but the saved Codex control session is stale or unreachable.'
    exit 1
}
