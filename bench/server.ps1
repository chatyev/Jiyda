# Локальный сайт для бенчмарков (bench/run.ps1): одна и та же страница-статья для всех
# браузеров, без сети. Слушает на 127.0.0.1…127.0.0.<Hosts>: каждая страница — со своего
# адреса, то есть отдельный сайт, как обычные вкладки на разных сайтах (браузеры делят
# процессы по сайтам, и одинаковый адрес у всех страниц исказил бы память).
# Когда страница загрузилась, она запрашивает /__ready?<метка> — сервер дописывает в журнал
# строку «<мс Unix> <метка>», по ней run.ps1 видит загрузку.
# Соединения обрабатываются параллельно: браузеры заранее открывают соединения «про запас»,
# и молчащее соединение не должно задерживать остальные.
param([int]$Port = 8790, [int]$Hosts = 50, [Parameter(Mandatory)][string]$Log)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System; using System.IO; using System.Net; using System.Net.Sockets; using System.Text; using System.Threading.Tasks;
public static class BenchServer {
    static byte[] page; static string log; static readonly object logLock = new object();
    public static void Run(int port, int hosts, string pagePath, string logPath) {
        page = File.ReadAllBytes(pagePath); log = logPath;
        // Только адреса обратной петли: брандмауэр Windows ничего не спрашивает.
        for (int i = 1; i <= hosts; i++) {
            var l = new TcpListener(IPAddress.Parse("127.0.0." + i), port);
            l.Start();
            Task.Run(() => Accept(l));
        }
    }
    static async Task Accept(TcpListener l) {
        while (true) {
            var c = await l.AcceptTcpClientAsync();
            _ = Task.Run(() => Serve(c));
        }
    }
    static async Task Serve(TcpClient c) {
        using (c) {
            try {
                var s = c.GetStream();
                var buf = new byte[8192];
                var read = s.ReadAsync(buf, 0, buf.Length);
                if (await Task.WhenAny(read, Task.Delay(10000)) != read) return;
                int n = read.Result;
                if (n <= 0) return;
                var line = Encoding.ASCII.GetString(buf, 0, n).Split(new[] { "\r\n" }, StringSplitOptions.None)[0];
                var parts = line.Split(' ');
                var target = parts.Length > 1 ? parts[1] : "/";
                if (target.StartsWith("/__ready")) {
                    var ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
                    var q = target.Contains("?") ? target.Substring(target.IndexOf('?') + 1) : "";
                    lock (logLock) File.AppendAllText(log, ms + " " + q + "\n");
                    await Send(s, "204 No Content", "text/plain", new byte[0]);
                } else if (target.StartsWith("/page.html")) {
                    await Send(s, "200 OK", "text/html; charset=utf-8", page);
                } else {
                    await Send(s, "404 Not Found", "text/plain", Encoding.ASCII.GetBytes("not found"));
                }
            } catch { }
        }
    }
    static async Task Send(NetworkStream s, string status, string type, byte[] body) {
        var head = Encoding.ASCII.GetBytes("HTTP/1.1 " + status + "\r\nContent-Type: " + type + "\r\nContent-Length: " + body.Length + "\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n");
        await s.WriteAsync(head, 0, head.Length);
        if (body.Length > 0) await s.WriteAsync(body, 0, body.Length);
    }
}
'@

[BenchServer]::Run($Port, $Hosts, (Join-Path $PSScriptRoot 'page.html'), $Log)
while ($true) { Start-Sleep -Seconds 3600 }
