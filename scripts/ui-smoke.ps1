# Проверка интерфейса без мыши и клавиатуры: запускает отладочный jiyda.exe с чистой
# папкой данных, открывает и закрывает каждый слой через канал автоматизации
# (automation.txt, см. src/app/session.rs) и проверяет журнал: ни одной ошибки JS,
# все слои открылись и закрылись, программа завершилась сама.
#
#   cargo build; pwsh scripts/ui-smoke.ps1
param(
    [string]$Exe = "target/debug/jiyda.exe",
    # С запасом: на сервере CI без видеокарты WebView2 рисует программно и заметно медленнее.
    [int]$TimeoutSec = 300,
    # Обычная рамка Windows вместо своей (без подкласса окна из frame.rs).
    [switch]$NativeFrame,
    [string]$ShotDir = (Join-Path ([IO.Path]::GetTempPath()) 'jiyda-smoke-shots')
)
$ErrorActionPreference = 'Stop'

$data = Join-Path ([IO.Path]::GetTempPath()) ("jiyda-smoke-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $data | Out-Null
$log = Join-Path $data 'jiyda.log'
$cmdFile = Join-Path $data 'automation.txt'
if ($NativeFrame) { Set-Content -Path (Join-Path $data 'config.json') -Value '{ "custom_titlebar": false }' -Encoding utf8NoBOM }
$label = if ($NativeFrame) { 'native frame' } else { 'Jiyda frame' }

# Сценарий в интерфейсе: команда → слой открыт → закрыть → слой закрыт.
$driver = @'
(async () => {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const until = async (ok, ms) => { const end = Date.now() + ms; while (!ok() && Date.now() < end) await wait(100); return ok(); };
  const steps = [['palette', 'palette'], ['orbit', 'orbit'], ['spaces', 'spaces'], ['history', 'history'],
    ['closed', 'closed'], ['ask', 'ask'], ['settings', 'settings'], ['shortcuts', 'settings'], ['downloads', 'downloads']];
  const failed = [], times = [];
  /* Для разбора сбоев: сколько сообщений ушло в ядро и пришло из него за шаг. */
  let cnt = {};
  const bump = (k) => { cnt[k] = (cnt[k] || 0) + 1; };
  S.watchPosts((m) => bump('>' + m.type));
  for (const k of ['render', 'command', 'data']) {
    const f = window.jiyda[k];
    window.jiyda[k] = (...a) => { bump('<' + (k === 'render' ? k : k + ':' + a[0])); return f.apply(window.jiyda, a); };
  }
  for (const [cmd, layer] of steps) {
    cnt = {};
    const t0 = Date.now();
    S.cmd(cmd);
    if (!(await until(() => S.layers.isOpen(layer), 15000))) failed.push(cmd + ' did not open');
    const t1 = Date.now();
    S.layers.close(layer);
    if (!(await until(() => !S.layers.isOpen(layer), 15000))) failed.push(cmd + ' did not close');
    await wait(300);
    times.push(cmd + ' ' + (t1 - t0) + 'ms');
    S.post({ type: 'log', msg: 'SMOKE step ' + cmd + ' ' + (t1 - t0) + 'ms ' + JSON.stringify(cnt) });
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

# При сбое: окна процесса Jiyda (класс, заголовок, видимость, размер, переднее ли) и снимок
# экрана в $ShotDir — CI прикладывает его к запуску.
function Snapshot {
    try {
        Add-Type -ErrorAction SilentlyContinue -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class SmokeWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern int GetWindowLongW(IntPtr h, int i);
    public struct RECT { public int L, T, R, B; }
    public static List<string> List(uint pid) {
        var fg = GetForegroundWindow(); var res = new List<string>();
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid) return true;
            var c = new StringBuilder(256); GetClassNameW(h, c, 256);
            var t = new StringBuilder(256); GetWindowTextW(h, t, 256);
            RECT r; GetWindowRect(h, out r);
            res.Add(string.Format("{0} class={1} title='{2}' visible={3} iconic={4} fg={5} rect={6},{7}-{8},{9} style={10:X8}",
                h, c, t, IsWindowVisible(h), IsIconic(h), h == fg, r.L, r.T, r.R, r.B, GetWindowLongW(h, -16)));
            return true;
        }, IntPtr.Zero);
        var fc = new StringBuilder(256); GetClassNameW(fg, fc, 256);
        res.Add("foreground: " + fg + " class=" + fc);
        return res;
    }
}
'@
        # Стеки всех потоков зависшего процесса (cdb из Windows SDK есть на серверах GitHub).
        $cdb = @("${env:ProgramFiles(x86)}\Windows Kits\10\Debuggers\x64\cdb.exe",
            "$env:ProgramFiles\Windows Kits\10\Debuggers\x64\cdb.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($cdb -and -not $proc.HasExited) {
            $sym = "$(Split-Path -Parent (Resolve-Path $Exe));srv*$env:TEMP\symbols*https://msdl.microsoft.com/download/symbols"
            Write-Host "--- стеки потоков ($label) ---"
            $out = Join-Path $data 'stacks.txt'
            $p = Start-Process -FilePath $cdb -PassThru -NoNewWindow -RedirectStandardOutput $out `
                -ArgumentList @('-pv', '-p', $proc.Id, '-y', "`"$sym`"", '-c', '"~* kn 40; qd"')
            if (-not $p.WaitForExit(120000)) { Stop-Process -Id $p.Id -Force; Write-Host '(cdb не уложился в 2 минуты)' }
            Get-Content $out -ErrorAction SilentlyContinue | Where-Object { $_ -notmatch '^ModLoad|^\*\*\*|^Symbol search|^Executable search|^\s*$' } |
                ForEach-Object { Write-Host $_ }
        } elseif (-not $cdb) { Write-Host '(cdb не найден — стеки не сняты)' }
        Write-Host "--- окна Jiyda ($label) ---"
        [SmokeWin]::List([uint32]$proc.Id) | ForEach-Object { Write-Host $_ }
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
        New-Item -ItemType Directory -Force -Path $ShotDir | Out-Null
        $file = Join-Path $ShotDir ("smoke-" + ($label -replace ' ', '-') + ".png")
        $bmp.Save($file); $g.Dispose(); $bmp.Dispose()
        Write-Host "снимок экрана: $file ($($b.Width)x$($b.Height))"
    } catch { Write-Host "снимок не удался: $_" }
}

$env:JIYDA_AUTOMATION = '1'
$env:JIYDA_DATA_DIR = $data
# Отладочный jiyda.exe — консольная программа и пишет журнал ещё и в консоль. Без перенаправления
# Start-Process открывает ей окно терминала; на сервере CI терминал перестаёт читать вывод, запись
# в консоль блокируется — и вместе с ней главный поток. Поэтому вывод — в файлы.
$proc = Start-Process -FilePath $Exe -PassThru -NoNewWindow `
    -RedirectStandardOutput (Join-Path $data 'stdout.txt') -RedirectStandardError (Join-Path $data 'stderr.txt')
$failure = $null
try {
    # Интерфейс готов, когда выполнил первый скрипт. Метка — в конце строки: иначе совпадёт
    # строка журнала, где ядро повторяет саму команду «js … SMOKE ready …».
    if (-not (WaitFor '(?m)SMOKE ready\r?$' { Send "js S.post({ type: 'log', msg: 'SMOKE ready' })" })) {
        $failure = 'интерфейс не загрузился'
        Snapshot
    } else {
        Send "js $driver"
        if (-not (WaitFor 'SMOKE (OK|FAIL)')) { $failure = 'сценарий не закончился' }
        elseif ((LogText) -match 'SMOKE FAIL: ([^\r\n]*)') { $failure = $Matches[1] }
        if ($failure) { Snapshot }
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
    Write-Host "UI smoke test FAILED ($label):`n$failure"
    Write-Host "--- jiyda.log ---"; Write-Host (LogText)
    exit 1
}
if ((LogText) -match 'SMOKE times: ([a-z][^\r\n]*)') { Write-Host "Steps: $($Matches[1])" }
Write-Host "UI smoke test passed ($label)"
Remove-Item -Recurse -Force $data -ErrorAction SilentlyContinue
