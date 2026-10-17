# Локальный сайт для тестов E2E: отдаёт файлы tests/e2e/site по http://localhost:<порт>/.
# .txt отдаётся как вложение (Content-Disposition: attachment) — для проверки загрузок.
# Запускает run.ps1; останавливается вместе с ним.
param([int]$Port = 8765, [string]$Root = (Join-Path $PSScriptRoot 'site'))
$ErrorActionPreference = 'Stop'

$types = @{ '.html' = 'text/html; charset=utf-8'; '.txt' = 'text/plain; charset=utf-8'; '.css' = 'text/css'; '.js' = 'text/javascript' }
$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add("http://localhost:$Port/")
$listener.Start()
Write-Host "e2e site: http://localhost:$Port/"
try {
    while ($listener.IsListening) {
        $ctx = $listener.GetContext()
        $res = $ctx.Response
        try {
            $rel = [Uri]::UnescapeDataString($ctx.Request.Url.AbsolutePath.TrimStart('/'))
            if (-not $rel) { $rel = 'index.html' }
            $file = [IO.Path]::GetFullPath((Join-Path $Root $rel))
            # Только файлы внутри site/.
            if (-not $file.StartsWith([IO.Path]::GetFullPath($Root)) -or -not (Test-Path $file -PathType Leaf)) {
                $res.StatusCode = 404
                $bytes = [Text.Encoding]::UTF8.GetBytes('not found')
            } else {
                $ext = [IO.Path]::GetExtension($file)
                $res.ContentType = $types[$ext] ?? 'application/octet-stream'
                if ($ext -eq '.txt') { $res.AddHeader('Content-Disposition', "attachment; filename=`"$([IO.Path]::GetFileName($file))`"") }
                $res.AddHeader('Cache-Control', 'no-store')
                $bytes = [IO.File]::ReadAllBytes($file)
            }
            $res.ContentLength64 = $bytes.Length
            $res.OutputStream.Write($bytes, 0, $bytes.Length)
        } catch {
            Write-Host "e2e site error: $_"
        } finally {
            $res.Close()
        }
    }
} finally {
    $listener.Stop()
}
