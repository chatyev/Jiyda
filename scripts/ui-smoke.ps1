# Проверка интерфейса без мыши и клавиатуры: запускает отладочный seyro.exe с чистой
# папкой данных, открывает и закрывает каждый слой через канал автоматизации
# (automation.txt, см. src/app/session.rs) и проверяет журнал: ни одной ошибки JS,
# все слои открылись и закрылись, программа завершилась сама.
#
#   cargo build; pwsh scripts/ui-smoke.ps1
param(
    [string]$Exe = "target/debug/seyro.exe",
    # С запасом: на сервере CI без видеокарты WebView2 рисует программно и заметно медленнее.
    [int]$TimeoutSec = 300
)
$ErrorActionPreference = 'Stop'

$data = Join-Path ([IO.Path]::GetTempPath()) ("seyro-smoke-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $data | Out-Null
$log = Join-Path $data 'seyro.log'
$cmdFile = Join-Path $data 'automation.txt'

# Сценарий в интерфейсе: команда → слой открыт → закрыть → слой закрыт.
$driver = @'
(async () => {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const until = async (ok, ms) => { const end = Date.now() + ms; while (!ok() && Date.now() < end) await wait(100); return ok(); };
  const steps = [['palette', 'palette'], ['orbit', 'orbit'], ['spaces', 'spaces'], ['history', 'history'],
    ['closed', 'closed'], ['ask', 'ask'], ['settings', 'settings'], ['shortcuts', 'settings'], ['downloads', 'downloads']];
  const failed = [], times = [];
  for (const [cmd, layer] of steps) {
    const t0 = Date.now();
    S.cmd(cmd);
    if (!(await until(() => S.layers.isOpen(layer), 15000))) failed.push(cmd + ' did not open');
    const t1 = Date.now();
    S.layers.close(layer);
    if (!(await until(() => !S.layers.isOpen(layer), 15000))) failed.push(cmd + ' did not close');
    await wait(300);
    times.push(cmd + ' ' + (t1 - t0) + 'ms');
  }
  S.cmd('theme'); await wait(300); S.cmd('theme'); await wait(300);
  S.post({ type: 'log', msg: 'SMOKE times: ' + times.join(', ') });
  S.post({ type: 'log', msg: 'SMOKE ' + (failed.length ? 'FAIL: ' + failed.join('; ') : 'OK') });
})();
'@ -replace "`r?`n\s*", ' '

function Send([string]$line) { Set-Content -Path $cmdFile -Value $line -Encoding utf8NoBOM }
function LogText { if (Test-Path $log) { Get-Content $log -Raw -Encoding utf8 } else { '' } }
function WaitFor([string]$pattern, [scriptblock]$retry) {
    $until = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $until) {
        if ((LogText) -match $pattern) { return $true }
        if ($proc.HasExited) { return $false }
        if ($retry) { & $retry }
        Start-Sleep -Milliseconds 500
    }
    $false
}

$env:SEYRO_AUTOMATION = '1'
$env:SEYRO_DATA_DIR = $data
$proc = Start-Process -FilePath $Exe -PassThru
$failure = $null
try {
    # Интерфейс готов, когда выполнил первый скрипт.
    if (-not (WaitFor 'SMOKE ready' { Send "js S.post({ type: 'log', msg: 'SMOKE ready' })" })) {
        $failure = 'интерфейс не загрузился'
    } else {
        Send "js $driver"
        if (-not (WaitFor 'SMOKE (OK|FAIL)')) { $failure = 'сценарий не закончился' }
        elseif ((LogText) -match 'SMOKE FAIL: ([^\r\n]*)') { $failure = $Matches[1] }
        Send 'cmd win_close'
        if (-not $proc.WaitForExit(60000)) { $failure = ($failure, 'программа не закрылась') -join '; ' }
    }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}

# Ошибки JS и паники: всё, что интерфейс прислал в журнал, кроме меток сценария.
$problems = (LogText) -split "`r?`n" | Where-Object { ($_ -match 'интерфейс: ' -and $_ -notmatch 'SMOKE') -or $_ -match 'паника' }
if ($problems) { $failure = (@($failure) + $problems | Where-Object { $_ }) -join "`n" }

if ($failure) {
    Write-Host "UI smoke test FAILED:`n$failure"
    Write-Host "--- seyro.log ---"; Write-Host (LogText)
    exit 1
}
if ((LogText) -match 'SMOKE times: ([a-z][^\r\n]*)') { Write-Host "Steps: $($Matches[1])" }
Write-Host 'UI smoke test passed'
Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
