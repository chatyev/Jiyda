# Jiyda в WinGet

Цель (план, раздел 47): `winget install Jiyda.Jiyda`.

Манифест версии 0.6.4 — в `0.6.4/` (формат 1.9.0: version, installer, defaultLocale).
Это **ещё не отправлено** в microsoft/winget-pkgs: отправка — публичное действие от имени
издателя, делается только после решения.

## Проверить у себя

```powershell
winget validate --manifest packaging\winget\0.6.4
# Установка из локального манифеста (нужно один раз: winget settings --enable LocalManifestFiles)
winget install --manifest packaging\winget\0.6.4
```

## Отправить

Проще всего — wingetcreate (сам делает форк и PR):

```powershell
winget install Microsoft.WingetCreate
wingetcreate submit --token <GitHub token> packaging\winget\0.6.4
```

Или вручную: форк https://github.com/microsoft/winget-pkgs, файлы — в
`manifests/j/Jiyda/Jiyda/0.6.4/`, PR. Проверка Microsoft запускает установщик в песочнице
и сканирует его; неподписанный установщик принимают, но подпись кода снимает
предупреждения SmartScreen и для пользователей WinGet.

## Новые версии

Для каждого выпуска — новая папка с версией, `InstallerUrl` и `InstallerSha256` (из
`JiydaSetup-<версия>.exe.sha256` в выпуске):

```powershell
wingetcreate update Jiyda.Jiyda --version 0.6.5 --urls https://github.com/chatyev/jiyda/releases/download/v0.6.5/JiydaSetup-0.6.5.exe --submit --token <токен>
```

Предрелизы (v0.6.5-beta.1) в WinGet не отправляем.

## Что проверить до отправки

- **License**: сейчас `Freeware` — до выбора лицензии репозитория (docs/license-options.md).
- **Publisher**: «Dmitriy Chatiev», как в Terms и Privacy.
- **Зависимость** `Microsoft.EdgeWebView2Runtime`: на Windows 11 он уже есть; на Windows
  10 установщик Jiyda проверяет его сам.
