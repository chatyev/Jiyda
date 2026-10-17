# Диагностика зависшего или упавшего Jiyda для run.ps1: окна процесса, снимок экрана, стеки потоков.
# Ждёт в области видимости: $proc (процесс Jiyda), $label, $ShotDir, $Exe, $data.

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

