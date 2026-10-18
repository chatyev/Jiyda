# Тесты E2E: настоящий Jiyda (отладочная сборка) с чистой папкой данных и локальным
# тестовым сайтом (tests/e2e/site). Сценарии выполняются в интерфейсе через канал
# автоматизации (JIYDA_AUTOMATION, src/app/session.rs).
#
#   cargo build
#   pwsh tests/e2e/run.ps1                     # все сценарии
#   pwsh tests/e2e/run.ps1 -Filter 'spaces'    # только подходящие по имени
#   pwsh tests/e2e/run.ps1 -NativeFrame        # с обычной рамкой Windows
#
# Два набора:
#   specs/*.spec.js            — обычные; перед каждым тестом t.reset();
#   restart/<имя>.before.spec.js + <имя>.after.spec.js — перезапуск: «before» готовит
#     состояние, Jiyda закрывается и открывается с тем же профилем, «after» проверяет,
#     что всё восстановилось (без t.reset()).
#
# Сбой: любой FAIL, ошибка JS в интерфейсе, паника, зависание или программа не закрылась.
# Отчёт JUnit — в $Report.
param(
    [string]$Exe = "target/debug/jiyda.exe",
    [string]$Filter = '',
    [int]$TimeoutSec = 900,
    [int]$Port = 8765,
    [switch]$NativeFrame,
    [string]$Report = '',
    # Своя папка сценариев — для отладки отдельного случая (перезапуски тогда не идут).
    [string]$SpecDir = (Join-Path $PSScriptRoot 'specs'),
    [string]$ShotDir = (Join-Path ([IO.Path]::GetTempPath()) 'jiyda-e2e-shots')
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
. (Join-Path $here 'diagnostics.ps1')
$label = if ($NativeFrame) { 'native frame' } else { 'Jiyda frame' }
$runs = Join-Path ([IO.Path]::GetTempPath()) ("jiyda-e2e-" + [guid]::NewGuid().ToString('N').Substring(0, 8))

# Новая папка данных: обновления не проверяются, загрузки — во временную папку, рамка — по ключу.
function New-Profile([string]$name) {
    $dir = Join-Path $runs $name
    New-Item -ItemType Directory -Path (Join-Path $dir 'downloads') -Force | Out-Null
    $config = @{ check_updates = $false; custom_titlebar = -not $NativeFrame; download_dir = (Join-Path $dir 'downloads') }
    Set-Content -Path (Join-Path $dir 'config.json') -Value ($config | ConvertTo-Json -Compress) -Encoding utf8NoBOM
    $dir
}

# Один запуск Jiyda: загрузить harness и сценарии, выполнить `$runCall`, закрыть.
# Возвращает строки журнала этого запуска и описание сбоев.
function Invoke-Session([string]$data, [string[]]$specFiles, [string]$runCall, [string]$name, [switch]$Kill) {
    $script:data = $data
    $log = Join-Path $data 'jiyda.log'
    $cmdFile = Join-Path $data 'automation.txt'
    $offset = if (Test-Path $log) { (Get-Item $log).Length } else { 0 }
    $text = {
        if (-not (Test-Path $log)) { return '' }
        $fs = [IO.File]::Open($log, 'Open', 'Read', 'ReadWrite')
        try { $fs.Position = $offset; (New-Object IO.StreamReader($fs, [Text.Encoding]::UTF8)).ReadToEnd() } finally { $fs.Dispose() }
    }
    # Jiyda читает и удаляет automation.txt каждые 250 мс; пока удаление не завершилось,
    # Windows не даёт создать файл заново («Access denied») — тогда повторяем.
    $send = {
        param([string[]]$lines)
        for ($i = 0; ; $i++) {
            try { Set-Content -Path $cmdFile -Value $lines -Encoding utf8NoBOM -ErrorAction Stop; return }
            catch { if ($i -ge 50) { throw } Start-Sleep -Milliseconds 100 }
        }
    }
    $wait = {
        param([string]$pattern, [int]$sec, [scriptblock]$retry)
        $until = (Get-Date).AddSeconds($sec)
        while ((Get-Date) -lt $until) {
            if ((& $text) -match $pattern) { return $true }
            if ($proc.HasExited) { return $false }
            if ($retry) { & $retry }
            Start-Sleep -Milliseconds 300
        }
        $false
    }
    $env:JIYDA_AUTOMATION = '1'
    $env:JIYDA_DATA_DIR = $data
    # Свой «Chrome» и «Edge» для переноса закладок (src/import.rs), а не браузеры этой машины.
    $env:JIYDA_IMPORT_ROOT = Join-Path $here 'import'
    # Вывод — в файлы: окно терминала на сервере CI может перестать читать его и остановить Jiyda.
    $file = $name -replace '[\\/:]', '-'
    $script:proc = Start-Process -FilePath $Exe -PassThru -NoNewWindow `
        -RedirectStandardOutput (Join-Path $data "stdout-$file.txt") -RedirectStandardError (Join-Path $data "stderr-$file.txt")
    $failure = @()
    try {
        if (-not (& $wait '(?m)E2E-READY\r?$' 60 { & $send "js S.post({ type: 'log', msg: 'E2E-READY' })" })) {
            $failure += "$name`: интерфейс не загрузился"
        } else {
            & $send (@("js window.E2E_BASE = 'http://localhost:$Port/'", "jsfile $(Join-Path $here 'harness.js')") +
                ($specFiles | ForEach-Object { "jsfile $_" }) + @("js $runCall"))
            if (-not (& $wait '(?m)E2E DONE [^\r\n]*$' $TimeoutSec)) { $failure += "$name`: сценарии не закончились" }
        }
        if ($failure) { $script:label = "$label, $name"; Snapshot }
        if ($Kill) {
            # Как сбой или «Снять задачу»: без сохранения при выходе.
            Stop-Process -Id $proc.Id -Force
            $proc.WaitForExit(10000) | Out-Null
        } else {
            & $send 'cmd win_close'
            if (-not $proc.WaitForExit(60000)) { $failure += "$name`: программа не закрылась" }
        }
    } finally {
        if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    }
    [pscustomobject]@{ Lines = (& $text) -split "`r?`n"; Failure = $failure }
}

$server = Start-Process pwsh -PassThru -NoNewWindow -ArgumentList @('-NoProfile', '-File', (Join-Path $here 'server.ps1'), '-Port', $Port) `
    -RedirectStandardOutput (Join-Path ([IO.Path]::GetTempPath()) "jiyda-e2e-server-$Port.txt") `
    -RedirectStandardError (Join-Path ([IO.Path]::GetTempPath()) "jiyda-e2e-server-$Port-err.txt")
$filterJs = $Filter -replace "'", "\'"
$sessions = @()
try {
    $specs = @(Get-ChildItem $SpecDir -Filter '*.spec.js' | Sort-Object Name | ForEach-Object FullName)
    if ($specs) { $sessions += Invoke-Session (New-Profile 'main') $specs "E2E.run('$filterJs')" 'main' }

    $restartDir = Join-Path $here 'restart'
    if ($SpecDir -eq (Join-Path $here 'specs') -and (Test-Path $restartDir)) {
        foreach ($before in Get-ChildItem $restartDir -Filter '*.before.spec.js' | Sort-Object Name) {
            $name = $before.Name -replace '\.before\.spec\.js$', ''
            $after = Join-Path $restartDir "$name.after.spec.js"
            $profile = New-Profile "restart-$name"
            # «// restart: kill» в файле — процесс убивается, а не закрывается (проверка сбоя).
            $kill = (Get-Content $before.FullName -Raw) -match '//\s*restart:\s*kill'
            $sessions += Invoke-Session $profile @($before.FullName) "E2E.run('$filterJs')" "$name/before" -Kill:$kill
            if (Test-Path $after) {
                $sessions += Invoke-Session $profile @($after) "E2E.run('$filterJs', { reset: false })" "$name/after"
            }
        }
    }
} finally {
    if (-not $server.HasExited) { Stop-Process -Id $server.Id -Force }
}

# Результаты: «E2E PASS <имя> (N мс)», «E2E FAIL <имя> ⟶ <причина>», «E2E SKIP <имя> ⟶ <причина>».
# В именах бывает «: », поэтому разделитель — «⟶».
$failure = @($sessions | ForEach-Object Failure)
$lines = @($sessions | ForEach-Object Lines)
$results = foreach ($l in $lines) {
    if ($l -match 'интерфейс: E2E (PASS|FAIL|SKIP) (.+)$') {
        $status, $rest = $Matches[1], $Matches[2]
        $name, $msg, $ms = $rest, '', 0
        if ($rest -match '^(.*) ⟶ (.*)$') { $name, $msg = $Matches[1], $Matches[2] }
        elseif ($rest -match '^(.*) \((\d+) мс\)$') { $name, $ms = $Matches[1], [int]$Matches[2] }
        [pscustomobject]@{ Status = $status; Name = $name; Ms = $ms; Message = $msg }
    }
}
$problems = $lines | Where-Object { ($_ -match 'интерфейс: ' -and $_ -notmatch 'интерфейс: E2E') -or $_ -match 'паника' }

foreach ($r in $results) {
    $mark = @{ PASS = '  ok  '; FAIL = ' FAIL '; SKIP = ' skip ' }[$r.Status]
    Write-Host ("[{0}] {1}{2}" -f $mark, $r.Name, ($(if ($r.Message) { " — $($r.Message)" } else { '' })))
}
$pass = @($results | Where-Object Status -eq 'PASS').Count
$fail = @($results | Where-Object Status -eq 'FAIL').Count
$skip = @($results | Where-Object Status -eq 'SKIP').Count
Write-Host "E2E ($label): $pass ok, $fail failed, $skip skipped"

# Сверка с итогами самого harness: разбор журнала не должен «потерять» упавший тест.
$done = foreach ($l in $lines) { if ($l -match 'E2E DONE pass=(\d+) fail=(\d+)') { [pscustomobject]@{ P = [int]$Matches[1]; F = [int]$Matches[2] } } }
$donePass = ($done | Measure-Object P -Sum).Sum; $doneFail = ($done | Measure-Object F -Sum).Sum
if ($done -and ($donePass -ne $pass -or $doneFail -ne $fail)) {
    $failure += "журнал разобран неверно: harness насчитал pass=$donePass fail=$doneFail, а run.ps1 — pass=$pass fail=$fail"
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
    if (-not $results) { Write-Host '--- журнал ---'; $lines | Select-Object -Last 80 | ForEach-Object { Write-Host $_ } }
    Write-Host "Данные запуска: $runs"
    exit 1
}
Remove-Item -Recurse -Force $runs -ErrorAction SilentlyContinue
