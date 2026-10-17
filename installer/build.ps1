# Собирает установщик target\installer\JiydaSetup-<версия>.exe:
#   powershell -ExecutionPolicy Bypass -File installer\build.ps1
# Нужен Inno Setup 6 (ISCC.exe): ищется в PATH и в обычных папках установки.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Jiyda $version"

$iscc = @(
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source,
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw "Не найден Inno Setup 6 (ISCC.exe): https://jrsoftware.org/isdl.php" }

Push-Location $root
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build завершился с ошибкой" }
    & $iscc "/DAppVersion=$version" "installer\jiyda.iss"
    if ($LASTEXITCODE -ne 0) { throw "ISCC завершился с ошибкой" }
} finally {
    Pop-Location
}
Get-Item "$root\target\installer\JiydaSetup-$version.exe"
