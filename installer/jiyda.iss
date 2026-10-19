; Установщик Jiyda (Inno Setup 6). Собирается скриптом installer/build.ps1
; или в GitHub Actions; версия передаётся из Cargo.toml: ISCC /DAppVersion=0.2.0 jiyda.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
; Числовая часть версии (0.6.5 из 0.6.5-beta.1): VersionInfoVersion принимает только числа.
#define AppNumVersion Copy(AppVersion, 1, Pos("-", AppVersion + "-") - 1)

[Setup]
; AppId нельзя менять: по нему Windows узнаёт, что это обновление уже установленного
; браузера (до 0.6.0 он назывался Seyro) — Jiyda ставится поверх, без второй копии.
AppId={{15EC25FD-92BA-4305-AA57-A90B43708E7E}
AppName=Jiyda
AppVersion={#AppVersion}
AppVerName=Jiyda {#AppVersion}
AppPublisher=Jiyda
VersionInfoVersion={#AppNumVersion}
; Для текущего пользователя, без прав администратора: %LOCALAPPDATA%\Programs\Jiyda.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Jiyda
DisableProgramGroupPage=yes
DisableDirPage=auto
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
SetupIconFile=..\assets\jiyda.ico
; Картинка справа вверху в окнах установщика — знак Jiyda вместо стандартной коробки.
WizardSmallImageFile=..\assets\jiyda-mark-128.png
UninstallDisplayIcon={app}\jiyda.exe
UninstallDisplayName=Jiyda
WizardStyle=modern
; Язык — по языку Windows, без лишнего окна выбора.
ShowLanguageDialog=auto
; Если Jiyda запущен, установщик закроет его перед заменой файлов.
CloseApplications=yes
RestartApplications=no
OutputDir=..\target\installer
OutputBaseFilename=JiydaSetup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
; После установки и удаления Windows обновит список браузеров и ассоциации файлов.
ChangesAssociations=yes

[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\target\release\jiyda.exe"; DestDir: "{app}"; Flags: ignoreversion
; Лицензии сторонних библиотек внутри jiyda.exe (cargo-about, см. about.toml).
Source: "..\target\third-party-licenses.html"; DestDir: "{app}"; Flags: ignoreversion

[InstallDelete]
; Остатки Seyro (до 0.6.0): программа, ярлыки. Его данные Jiyda переносит сам при запуске.
Type: filesandordirs; Name: "{autopf}\Seyro"
Type: files; Name: "{autoprograms}\Seyro.lnk"
Type: files; Name: "{autodesktop}\Seyro.lnk"

[Icons]
Name: "{autoprograms}\Jiyda"; Filename: "{app}\jiyda.exe"
Name: "{autodesktop}\Jiyda"; Filename: "{app}\jiyda.exe"; Tasks: desktopicon

[Registry]
; Jiyda в списке браузеров Windows («Параметры → Приложения по умолчанию»).
; Внутренние имена (SeyroHTML, StartMenuInternet\Seyro, RegisteredApplications\Seyro)
; остались от Seyro нарочно: на них ссылается выбор браузера по умолчанию, и тем, у кого
; Seyro был браузером по умолчанию, не придётся выбирать заново. Видно только «Jiyda».
; Root HKA — для установки без прав администратора это HKCU.
; Тип документа, через который Windows открывает в Jiyda ссылки и файлы.
Root: HKA; Subkey: "Software\Classes\SeyroHTML"; ValueType: string; ValueData: "Jiyda HTML Document"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\SeyroHTML"; ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "Jiyda HTML Document"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Jiyda"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\Application"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\jiyda.exe,0"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\DefaultIcon"; ValueType: string; ValueData: "{app}\jiyda.exe,0"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\shell\open\command"; ValueType: string; ValueData: """{app}\jiyda.exe"" ""%1"""
; Описание браузера и что он умеет открывать.
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro"; ValueType: string; ValueData: "Jiyda"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\DefaultIcon"; ValueType: string; ValueData: "{app}\jiyda.exe,0"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\shell\open\command"; ValueType: string; ValueData: """{app}\jiyda.exe"""
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Jiyda"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Jiyda — браузер с вертикальными вкладками и пространствами"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\jiyda.exe,0"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\StartMenu"; ValueType: string; ValueName: "StartMenuInternet"; ValueData: "Seyro"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\URLAssociations"; ValueType: string; ValueName: "http"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\URLAssociations"; ValueType: string; ValueName: "https"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".htm"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".html"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".xhtml"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".svg"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".pdf"; ValueData: "SeyroHTML"
; По этому имени Jiyda ищет себя в списке (instance.rs) и открывает свою страницу в «Параметрах».
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "Seyro"; ValueData: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; Flags: uninsdeletevalue
[Run]
Filename: "{app}\jiyda.exe"; Description: "{cm:LaunchProgram,Jiyda}"; Flags: nowait postinstall skipifsilent
; Назначить браузер по умолчанию молча Windows не даёт — открываем страницу Jiyda в «Параметрах».
Filename: "ms-settings:defaultapps?registeredAppUser=Seyro"; Description: "Сделать Jiyda браузером по умолчанию"; Flags: shellexec nowait postinstall skipifsilent unchecked
; Обновление из самого Jiyda (update.rs запускает установщик с /update=1): открыть его снова.
Filename: "{app}\jiyda.exe"; Flags: nowait; Check: IsUpdate

; Данные браузера (%LOCALAPPDATA%\Jiyda: вкладки, пространства, логины) при удалении
; остаются — как у других браузеров; при повторной установке всё вернётся.

[Code]
const
  WebView2Id = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  WebView2Url = 'https://developer.microsoft.com/microsoft-edge/webview2/';

// WebView2 Runtime установлен (на Windows 11 — всегда), если в реестре есть его версия.
function WebView2Installed: Boolean;
var
  Version: String;
begin
  Result :=
    (RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\' + WebView2Id, 'pv', Version) or
     RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\' + WebView2Id, 'pv', Version) or
     RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\' + WebView2Id, 'pv', Version))
    and (Version <> '') and (Version <> '0.0.0.0');
end;

// Установщик запущен из Jiyda для обновления.
function IsUpdate: Boolean;
begin
  Result := ExpandConstant('{param:update|0}') = '1';
end;

function InitializeSetup: Boolean;
var
  ErrorCode: Integer;
begin
  Result := True;
  if not WebView2Installed then
    if MsgBox('Для работы Jiyda нужен Microsoft Edge WebView2 Runtime, а он не найден.' + #13#10 +
              'Открыть страницу загрузки? Установку Jiyda можно продолжить и поставить WebView2 потом.',
              mbConfirmation, MB_YESNO) = IDYES then
      ShellExec('open', WebView2Url, '', '', SW_SHOWNORMAL, ewNoWait, ErrorCode);
end;
