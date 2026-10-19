# Бенчмарки Jiyda против Microsoft Edge и Google Chrome (план, раздел 32).
#
#   cargo build --release
#   pwsh bench/run.ps1                                  # все браузеры, 1/10/50 страниц, 3 прогона
#   pwsh bench/run.ps1 -Browsers jiyda,chrome -Runs 5
#
# Как меряем (одинаково для всех браузеров):
# - каждый прогон — новый пустой профиль, браузер открывает N одинаковых страниц
#   (bench/page.html — статья с текстом, таблицами и картинками) с локального сервера,
#   без сети, каждую — со своего адреса 127.0.0.<k>, то есть как N разных сайтов (браузеры
#   делят процессы по сайтам); перед замерами — один прогрев, он не считается;
# - «запуск» — от старта процесса до загрузки первой страницы (событие load);
# - «все загружены» — до загрузки последней из N страниц;
# - память — сумма рабочих наборов (Working Set) и частной памяти (Private Bytes) всех
#   процессов браузера (главный, процессы страниц, GPU и т.п.), через $Settle с после
#   загрузки;
# - процессор в простое — сколько процессорного времени все эти процессы потратили за
#   $IdleSeconds с, когда ничего не происходит, в процентах от одного ядра;
# - итог — медиана прогонов.
# Не меряем: батарею и долю сессий без сбоев (нужны долгие наблюдения на ноутбуках).
param(
    [ValidateSet('jiyda', 'edge', 'chrome')][string[]]$Browsers = @('jiyda', 'edge', 'chrome'),
    [int[]]$Tabs = @(1, 10, 50),
    [int]$Runs = 3,
    [int]$Port = 8790,
    [int]$Settle = 10,
    [int]$IdleSeconds = 20,
    [string]$Out = (Join-Path $PSScriptRoot 'results')
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$work = Join-Path ([IO.Path]::GetTempPath()) ('jiyda-bench-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force -Path $work, $Out | Out-Null
$log = Join-Path $work 'ready.log'
Set-Content -Path $log -Value '' -NoNewline

function Find-Exe([string[]]$paths) { $paths | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1 }
$exes = @{
    jiyda  = Find-Exe @((Join-Path $root 'target\release\jiyda.exe'))
    edge   = Find-Exe @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe")
    chrome = Find-Exe @("$env:ProgramFiles\Google\Chrome\Application\chrome.exe", "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe", "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe")
}
foreach ($b in $Browsers) { if (-not $exes[$b]) { throw "Не найден $b. Для Jiyda: cargo build --release" } }

$hosts = ($Tabs | Measure-Object -Maximum).Maximum
$server = Start-Process pwsh -PassThru -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'server.ps1'), '-Port', $Port, '-Hosts', $hosts, '-Log', $log)
Start-Sleep -Seconds 1

# Все процессы браузера: корень, всё, в чьей командной строке есть папка профиля (процессы
# Chromium и WebView2 получают её ключом --user-data-dir), и все их потомки.
function Get-Tree([int]$rootPid, [string]$profileDir) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, CommandLine
    $ids = [Collections.Generic.HashSet[int]]::new()
    if ($rootPid) { [void]$ids.Add($rootPid) }
    foreach ($p in $all) { if ($p.CommandLine -and $p.CommandLine.Contains($profileDir)) { [void]$ids.Add([int]$p.ProcessId) } }
    do {
        $added = $false
        foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)) { $added = $true } }
    } while ($added)
    $ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
}

function Start-Browser([string]$b, [string]$profileDir, [string[]]$urls) {
    New-Item -ItemType Directory -Force -Path $profileDir | Out-Null
    if ($b -eq 'jiyda') {
        # Без знакомства и предложения переноса; обновления в отдельной папке данных не проверяются.
        Set-Content -Path (Join-Path $profileDir 'config.json') -Value '{"check_updates":false,"import_offered":true}' -Encoding utf8NoBOM
        $env:JIYDA_DATA_DIR = $profileDir
        try { return Start-Process $exes[$b] -PassThru -ArgumentList $urls } finally { Remove-Item Env:JIYDA_DATA_DIR }
    }
    $flags = @("--user-data-dir=`"$profileDir`"", '--no-first-run', '--no-default-browser-check', '--disable-sync', '--window-size=1280,800')
    Start-Process $exes[$b] -PassThru -ArgumentList ($flags + $urls)
}

function Measure-Run([string]$b, [int]$n, [string]$label) {
    $profileDir = Join-Path $work "$label-profile"
    # Каждая страница — со своего адреса 127.0.0.<k>: отдельный сайт (server.ps1).
    $urls = 1..$n | ForEach-Object { "http://127.0.0.${_}:$Port/page.html?$label-$_" }
    $t0 = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $proc = Start-Browser $b $profileDir $urls
    $deadline = (Get-Date).AddSeconds(180)
    do {
        Start-Sleep -Milliseconds 200
        $hits = @(Get-Content $log | Where-Object { $_ -match " $label-\d+$" } | ForEach-Object { [long]($_ -split ' ')[0] })
    } while ($hits.Count -lt $n -and (Get-Date) -lt $deadline)
    $r = [ordered]@{ browser = $b; tabs = $n; loaded = $hits.Count }
    if ($hits.Count) {
        $r.first_ms = ($hits | Measure-Object -Minimum).Minimum - $t0
        $r.all_ms = ($hits | Measure-Object -Maximum).Maximum - $t0
    }
    Start-Sleep -Seconds $Settle
    $tree = @(Get-Tree $proc.Id $profileDir)
    $r.processes = $tree.Count
    $r.ws_mb = [math]::Round(($tree | Measure-Object WorkingSet64 -Sum).Sum / 1MB, 1)
    $r.private_mb = [math]::Round(($tree | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB, 1)
    # Процессорное время — по каждому процессу, который жив и в начале, и в конце (процесс,
    # завершившийся за это время, иначе «вычел» бы своё время из суммы).
    $cpu0 = @{}
    foreach ($p in $tree) { try { $cpu0[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds } catch { } }
    Start-Sleep -Seconds $IdleSeconds
    $tree2 = @(Get-Tree $proc.Id $profileDir)
    $spent = 0.0
    foreach ($p in $tree2) { if ($cpu0.ContainsKey($p.Id)) { try { $spent += $p.TotalProcessorTime.TotalMilliseconds - $cpu0[$p.Id] } catch { } } }
    $r.idle_cpu_pct = [math]::Round(100 * $spent / ($IdleSeconds * 1000), 2)
    foreach ($p in $tree2) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 2
    for ($i = 0; $i -lt 10 -and (Test-Path $profileDir); $i++) { Remove-Item -Recurse -Force $profileDir -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 500 }
    [pscustomobject]$r
}

function Median([double[]]$xs) {
    $s = @($xs | Where-Object { $_ -ne $null } | Sort-Object)
    if ($s.Count -eq 0) { return $null }
    $m = [math]::Floor($s.Count / 2)
    if ($s.Count % 2) { $s[$m] } else { ($s[$m - 1] + $s[$m]) / 2 }
}

$results = @()
try {
    foreach ($b in $Browsers) {
        Write-Host "== ${b}: прогрев"
        [void](Measure-Run $b 1 "$b-warmup")
        foreach ($n in $Tabs) {
            for ($run = 1; $run -le $Runs; $run++) {
                $r = Measure-Run $b $n "$b-$n-$run"
                Write-Host ("{0} {1,2} стр. прогон {2}: загружено {3}, первая {4} мс, все {5} мс, WS {6} МБ, private {7} МБ, CPU {8}%" -f $b, $n, $run, $r.loaded, $r.first_ms, $r.all_ms, $r.ws_mb, $r.private_mb, $r.idle_cpu_pct)
                $results += $r
            }
        }
    }
} finally {
    if (-not $server.HasExited) { Stop-Process -Id $server.Id -Force }
}

# ---------- Итог ----------
function Ver([string]$exe) { if ($exe) { (Get-Item $exe).VersionInfo.ProductVersion } }
$wv2 = @('HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}') |
    ForEach-Object { (Get-ItemProperty $_ -ErrorAction SilentlyContinue).pv } | Select-Object -First 1
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$env_ = [ordered]@{
    date = (Get-Date).ToString('yyyy-MM-dd')
    cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
    ram_gb = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
    windows = "$($cv.ProductName) $($cv.DisplayVersion) (build $($cv.CurrentBuild))"
    jiyda = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    webview2 = $wv2
    edge = Ver $exes.edge
    chrome = Ver $exes.chrome
    runs = $Runs; settle_s = $Settle; idle_s = $IdleSeconds
}
$summary = foreach ($b in $Browsers) {
    foreach ($n in $Tabs) {
        $rs = $results | Where-Object { $_.browser -eq $b -and $_.tabs -eq $n }
        [pscustomobject][ordered]@{
            browser = $b; tabs = $n
            startup_ms = if ($n -eq 1) { Median ($rs.first_ms) } else { $null }
            all_loaded_ms = Median ($rs.all_ms)
            ws_mb = Median ($rs.ws_mb)
            private_mb = Median ($rs.private_mb)
            idle_cpu_pct = Median ($rs.idle_cpu_pct)
            loaded_ok = ($rs | Where-Object { $_.loaded -eq $n }).Count -eq $rs.Count
        }
    }
}
$stamp = $env_.date
$json = Join-Path $Out "$stamp.json"
[ordered]@{ environment = $env_; summary = $summary; runs = $results } | ConvertTo-Json -Depth 5 | Set-Content -Path $json -Encoding utf8NoBOM
$md = @("# Jiyda benchmarks — $stamp", '',
    "$($env_.cpu), $($env_.ram_gb) GB RAM, $($env_.windows).",
    "Jiyda $($env_.jiyda) (WebView2 $($env_.webview2)), Microsoft Edge $($env_.edge), Google Chrome $($env_.chrome).",
    "Median of $Runs runs; memory $Settle s after the last page loaded; idle CPU over $IdleSeconds s. Method: bench/run.ps1.", '',
    '| Browser | Pages | Start to first page, ms | All pages loaded, ms | Working set, MB | Private, MB | Idle CPU, % of one core |',
    '|---|---:|---:|---:|---:|---:|---:|')
$md += $summary | ForEach-Object { "| $($_.browser) | $($_.tabs) | $($_.startup_ms ?? '—') | $($_.all_loaded_ms) | $($_.ws_mb) | $($_.private_mb) | $($_.idle_cpu_pct) |" }
$md | Set-Content -Path (Join-Path $Out "$stamp.md") -Encoding utf8NoBOM
$md | ForEach-Object { Write-Host $_ }
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
