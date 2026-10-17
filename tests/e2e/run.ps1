# Тесты E2E: настоящий Jiyda (отладочная сборка) с чистой папкой данных и локальным
# тестовым сайтом (tests/e2e/site). Сценарии — tests/e2e/specs/*.spec.js, выполняются в
# интерфейсе через канал автоматизации (JIYDA_AUTOMATION, src/app/session.rs).
#
#   cargo build
#   pwsh tests/e2e/run.ps1                     # все сценарии
#   pwsh tests/e2e/run.ps1 -Filter 'spaces'    # только подходящие по имени
#   pwsh tests/e2e/run.ps1 -NativeFrame        # с обычной рамкой Windows
#
# Сбой: любой FAIL, ошибка JS в интерфейсе, паника, зависание или программа не закрылась.
# Отчёт JUnit — в $Report (CI показывает его во вкладке тестов).
param(
    [string]$Exe = "target/debug/jiyda.exe",
    [string]$Filter = '',
    [int]$TimeoutSec = 900,
    [int]$Port = 8765,
    [switch]$NativeFrame,
    [string]$Report = '',
    # Своя папка сценариев — для отладки отдельного случая.
    [string]$SpecDir = (Join-Path $PSScriptRoot 'specs'),
    [string]$ShotDir = (Join-Path ([IO.Path]::GetTempPath()) 'jiyda-e2e-shots')
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
. (Join-Path $here 'diagnostics.ps1')

$data = Join-Path ([IO.Path]::GetTempPath()) ("jiyda-e2e-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $data | Out-Null
$log = Join-Path $data 'jiyda.log'
$cmdFile = Join-Path $data 'automation.txt'
$label = if ($NativeFrame) { 'native frame' } else { 'Jiyda frame' }
# Тестам не нужна проверка обновлений; загрузки — во временную папку запуска, не в «Загрузки».
$downloads = Join-Path $data 'downloads'
New-Item -ItemType Directory -Path $downloads | Out-Null
$config = @{ check_updates = $false; custom_titlebar = -not $NativeFrame; download_dir = $downloads } | ConvertTo-Json -Compress
Set-Content -Path (Join-Path $data 'config.json') -Value $config -Encoding utf8NoBOM

function Send([string[]]$lines) { Set-Content -Path $cmdFile -Value $lines -Encoding utf8NoBOM }
function LogText { if (Test-Path $log) { Get-Content $log -Raw -Encoding utf8 } else { '' } }
function WaitFor([string]$pattern, [int]$sec, [scriptblock]$retry) {
    $until = (Get-Date).AddSeconds($sec)
    while ((Get-Date) -lt $until) {
        if ((LogText) -match $pattern) { return $true }
        if ($proc.HasExited) { return $false }
        if ($retry) { & $retry }
        Start-Sleep -Milliseconds 300
    }
    $false
}

$server = Start-Process pwsh -PassThru -NoNewWindow -ArgumentList @('-NoProfile', '-File', (Join-Path $here 'server.ps1'), '-Port', $Port) `
    -RedirectStandardOutput (Join-Path $data 'server.txt') -RedirectStandardError (Join-Path $data 'server-err.txt')
$env:JIYDA_AUTOMATION = '1'
$env:JIYDA_DATA_DIR = $data
# Вывод — в файлы: окно терминала на сервере CI может перестать читать его и остановить Jiyda.
$proc = Start-Process -FilePath $Exe -PassThru -NoNewWindow `
    -RedirectStandardOutput (Join-Path $data 'stdout.txt') -RedirectStandardError (Join-Path $data 'stderr.txt')
$failure = @()
try {
    if (-not (WaitFor '(?m)E2E-READY\r?$' 60 { Send "js S.post({ type: 'log', msg: 'E2E-READY' })" })) {
        $failure += 'интерфейс не загрузился'
    } else {
        $specs = Get-ChildItem $SpecDir -Filter '*.spec.js' | Sort-Object Name
        $filterJs = $Filter -replace "'", "\'"
        Send (@("js window.E2E_BASE = 'http://localhost:$Port/'", "jsfile $(Join-Path $here 'harness.js')") +
            ($specs | ForEach-Object { "jsfile $($_.FullName)" }) + @("js E2E.run('$filterJs')"))
        if (-not (WaitFor '(?m)E2E DONE [^\r\n]*$' $TimeoutSec)) { $failure += 'сценарии не закончились' }
    }
    if ($failure) { Snapshot }
    Send 'cmd win_close'
    if (-not $proc.WaitForExit(60000)) { $failure += 'программа не закрылась' }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    if (-not $server.HasExited) { Stop-Process -Id $server.Id -Force }
}

# Результаты сценариев и ошибки интерфейса из журнала.
$lines = (LogText) -split "`r?`n"
# Формат строк (harness.js): «E2E PASS <имя> (N мс)», «E2E FAIL <имя> ⟶ <причина>»,
# «E2E SKIP <имя> ⟶ <причина>». В именах бывает «: », поэтому разделитель — «⟶».
$results = foreach ($l in $lines) {
    if ($l -match 'интерфейс: E2E (PASS|FAIL|SKIP) (.+)$') {
        $status, $rest = $Matches[1], $Matches[2]
        $name, $msg, $ms = $rest, '', 0
        if ($rest -match '^(.*) ⟶ (.*)$') { $name, $msg = $Matches[1], $Matches[2] }
        elseif ($rest -match '^(.*) \((\d+) мс\)$') { $name, $ms = $Matches[1], [int]$Matches[2] }
        [pscustomobject]@{ Status = $status; Name = $name; Ms = $ms; Message = $msg }
    }
}
# Сверка с итогом самого harness: разбор журнала не должен «потерять» упавший тест.
$done = if ((LogText) -match 'E2E DONE pass=(\d+) fail=(\d+) skip=(\d+)') { @{ Pass = [int]$Matches[1]; Fail = [int]$Matches[2] } }
$problems = $lines | Where-Object { ($_ -match 'интерфейс: ' -and $_ -notmatch 'интерфейс: E2E') -or $_ -match 'паника' }

foreach ($r in $results) {
    $mark = @{ PASS = '  ok  '; FAIL = ' FAIL '; SKIP = ' skip ' }[$r.Status]
    Write-Host ("[{0}] {1}{2}" -f $mark, $r.Name, ($(if ($r.Message) { " — $($r.Message)" } else { '' })))
}
$pass = @($results | Where-Object Status -eq 'PASS').Count
$fail = @($results | Where-Object Status -eq 'FAIL').Count
$skip = @($results | Where-Object Status -eq 'SKIP').Count
Write-Host "E2E ($label): $pass ok, $fail failed, $skip skipped"
if ($done -and ($done.Pass -ne $pass -or $done.Fail -ne $fail)) {
    $failure += "журнал разобран неверно: harness насчитал pass=$($done.Pass) fail=$($done.Fail), а run.ps1 — pass=$pass fail=$fail"
}

if ($Report) {
    $esc = { param($s) [Security.SecurityElement]::Escape([string]$s) }
    $cases = foreach ($r in $results) {
        $body = switch ($r.Status) { 'FAIL' { "<failure message=`"$(& $esc $r.Message)`"/>" } 'SKIP' { '<skipped/>' } default { '' } }
        "  <testcase classname=`"e2e.$(& $esc ($r.Name -split ':')[0])`" name=`"$(& $esc $r.Name)`" time=`"$($r.Ms / 1000)`">$body</testcase>"
    }
    $errors = foreach ($p in @($failure) + @($problems)) { "  <testcase classname=`"e2e.harness`" name=`"$(& $esc $p)`"><error/></testcase>" }
    New-Item -ItemType Directory -Force -Path (Split-Path $Report) | Out-Null
    @('<?xml version="1.0" encoding="UTF-8"?>',
        "<testsuite name=`"e2e ($label)`" tests=`"$(@($results).Count + @($errors).Count)`" failures=`"$fail`" errors=`"$(@($errors).Count)`" skipped=`"$skip`">") +
        $cases + $errors + '</testsuite>' | Set-Content -Path $Report -Encoding utf8
}

if ($failure -or $problems -or $fail -or -not $results) {
    if ($failure) { Write-Host "Сбой: $($failure -join '; ')" }
    if ($problems) { Write-Host "Ошибки интерфейса:"; $problems | ForEach-Object { Write-Host "  $_" } }
    if (-not $results) { Write-Host '--- jiyda.log ---'; Write-Host (LogText) }
    Write-Host "Данные запуска: $data"
    exit 1
}
Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
