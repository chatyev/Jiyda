; Установщик Seyro (Inno Setup 6). Собирается скриптом installer/build.ps1
; или в GitHub Actions; версия передаётся из Cargo.toml: ISCC /DAppVersion=0.2.0 seyro.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; AppId нельзя менять: по нему Windows узнаёт, что это обновление уже установленного Seyro.
AppId={{15EC25FD-92BA-4305-AA57-A90B43708E7E}
AppName=Seyro
AppVersion={#AppVersion}
AppVerName=Seyro {#AppVersion}
AppPublisher=Seyro
VersionInfoVersion={#AppVersion}
; Для текущего пользователя, без прав администратора: %LOCALAPPDATA%\Programs\Seyro.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Seyro
DisableProgramGroupPage=yes
DisableDirPage=auto
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
SetupIconFile=..\assets\seyro.ico
; Картинка справа вверху в окнах установщика — знак Seyro вместо стандартной коробки.
WizardSmallImageFile=..\assets\seyro-mark-128.png
UninstallDisplayIcon={app}\seyro.exe
UninstallDisplayName=Seyro
WizardStyle=modern
; Язык — по языку Windows, без лишнего окна выбора.
ShowLanguageDialog=auto
; Если Seyro запущен, установщик закроет его перед заменой файлов.
CloseApplications=yes
RestartApplications=no
OutputDir=..\target\installer
OutputBaseFilename=SeyroSetup-{#AppVersion}
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
Source: "..\target\release\seyro.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Seyro"; Filename: "{app}\seyro.exe"
Name: "{autodesktop}\Seyro"; Filename: "{app}\seyro.exe"; Tasks: desktopicon

[Registry]
; Seyro в списке браузеров Windows («Параметры → Приложения по умолчанию»).
; Root HKA — для установки без прав администратора это HKCU.
; Тип документа, через который Windows открывает в Seyro ссылки и файлы.
Root: HKA; Subkey: "Software\Classes\SeyroHTML"; ValueType: string; ValueData: "Seyro HTML Document"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\SeyroHTML"; ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "Seyro HTML Document"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\Application"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Seyro"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\Application"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\seyro.exe,0"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\DefaultIcon"; ValueType: string; ValueData: "{app}\seyro.exe,0"
Root: HKA; Subkey: "Software\Classes\SeyroHTML\shell\open\command"; ValueType: string; ValueData: """{app}\seyro.exe"" ""%1"""
; Описание браузера и что он умеет открывать.
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro"; ValueType: string; ValueData: "Seyro"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\DefaultIcon"; ValueType: string; ValueData: "{app}\seyro.exe,0"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\shell\open\command"; ValueType: string; ValueData: """{app}\seyro.exe"""
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Seyro"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Seyro — браузер с вертикальными вкладками и пространствами"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\seyro.exe,0"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\StartMenu"; ValueType: string; ValueName: "StartMenuInternet"; ValueData: "Seyro"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\URLAssociations"; ValueType: string; ValueName: "http"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\URLAssociations"; ValueType: string; ValueName: "https"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".htm"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".html"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".xhtml"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".svg"; ValueData: "SeyroHTML"
Root: HKA; Subkey: "Software\Clients\StartMenuInternet\Seyro\Capabilities\FileAssociations"; ValueType: string; ValueName: ".pdf"; ValueData: "SeyroHTML"
; По этому имени Seyro ищет себя в списке (instance.rs) и открывает свою страницу в «Параметрах».
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "Seyro"; ValueData: "Software\Clients\StartMenuInternet\Seyro\Capabilities"; Flags: uninsdeletevalue
[Run]
Filename: "{app}\seyro.exe"; Description: "{cm:LaunchProgram,Seyro}"; Flags: nowait postinstall skipifsilent
; Назначить браузер по умолчанию молча Windows не даёт — открываем страницу Seyro в «Параметрах».
Filename: "ms-settings:defaultapps?registeredAppUser=Seyro"; Description: "Сделать Seyro браузером по умолчанию"; Flags: shellexec nowait postinstall skipifsilent unchecked

; Данные браузера (%LOCALAPPDATA%\Seyro: вкладки, пространства, логины) при удалении
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

function InitializeSetup: Boolean;
var
  ErrorCode: Integer;
begin
  Result := True;
  if not WebView2Installed then
    if MsgBox('Для работы Seyro нужен Microsoft Edge WebView2 Runtime, а он не найден.' + #13#10 +
              'Открыть страницу загрузки? Установку Seyro можно продолжить и поставить WebView2 потом.',
              mbConfirmation, MB_YESNO) = IDYES then
      ShellExec('open', WebView2Url, '', '', SW_SHOWNORMAL, ewNoWait, ErrorCode);
end;
