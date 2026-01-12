; Copyright (C) 2017-2026 Smart Code OOD 203358507

#define MyAppName "Stremio Service"
#define MyAppShortName "StremioService"
#define MyAppExeName "stremio-service.exe"
#define MyAppRoot SourcePath + "..\"
#define MyAppBinLocation SourcePath + "..\stremio-service-windows\"
#define MyAppResBinLocation SourcePath + "..\resources\bin\windows\"
#define MyAppExeLocation MyAppBinLocation + MyAppExeName
#define MyAppVersion() GetVersionComponents(MyAppExeLocation, Local[0], Local[1], Local[2], Local[3]), \
  Str(Local[0]) + "." + Str(Local[1]) + "." + Str(Local[2])

#define MyAppPublisher "Smart Code OOD"
#define MyAppCopyright "Copyright (C) 2017-" + GetDateTimeString('yyyy', '', '') + " " + MyAppPublisher
#define MyAppURL "https://www.stremio.com/"
#define MyAppGoodbyeURL "https://www.strem.io/goodbye"

[Setup]
; NOTE: The value of AppId uniquely identifies this application. Do not use the same AppId value in installers for other applications.
; (To generate a new GUID, click Tools | Generate GUID inside the IDE.)
AppId={{DD3870DA-AF3C-4C73-B010-72944AB610C6}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppCopyright={#MyAppCopyright} 
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppShortName}
SetupMutex=StremioServiceSetupMutex,Global\StremioServiceSetupMutex
; Remove the following line to run in administrative install mode (install for all users.)
PrivilegesRequired=lowest
DisableReadyPage=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
; DisableFinishedPage=yes
ChangesAssociations=yes
OutputBaseFilename={#MyAppShortName}Setup
OutputDir=..
Compression=lzma
SolidCompression=yes
WizardStyle=modern
LanguageDetectionMethod=uilanguage
ShowLanguageDialog=auto
CloseApplications=yes
WizardImageFile={#SourcePath}\windows-installer.bmp
WizardSmallImageFile={#SourcePath}\windows-installer-header.bmp
SetupIconFile={#SourcePath}..\resources\service.ico
UninstallDisplayIcon={app}\{#MyAppExeName},0
SignTool=stremiosign
SignedUninstaller=yes

[Code]
function ShouldSkipPage(PageID: Integer): Boolean;
begin
  { Hide finish page if run app is selected }
  if (PageID = wpFinished) and WizardIsTaskSelected('runapp') then
    Result := True
  else
    Result := False;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  case (CurPageID) of
    wpSelectTasks: WizardForm.NextButton.Caption := SetupMessage(msgButtonInstall);
    wpFinished: WizardForm.NextButton.Caption := SetupMessage(msgButtonFinish);
  else
    WizardForm.NextButton.Caption := SetupMessage(msgButtonNext);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
begin
  if (CurStep = ssDone) and WizardIsTaskSelected('runapp') then
    ExecAsOriginalUser(ExpandConstant('{app}\{#MyAppExeName}'), '', '', SW_SHOW, ewNoWait, ResultCode);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ErrorCode: Integer;
begin
  case (CurUninstallStep) of
    usDone: ShellExec('', ExpandConstant('{#MyAppGoodbyeURL}'), '', '', SW_SHOW, ewNoWait, ErrorCode);
  end;
end;

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "armenian"; MessagesFile: "compiler:Languages\Armenian.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "bulgarian"; MessagesFile: "compiler:Languages\Bulgarian.isl"
Name: "catalan"; MessagesFile: "compiler:Languages\Catalan.isl"
Name: "corsican"; MessagesFile: "compiler:Languages\Corsican.isl"
Name: "czech"; MessagesFile: "compiler:Languages\Czech.isl"
Name: "danish"; MessagesFile: "compiler:Languages\Danish.isl"
Name: "dutch"; MessagesFile: "compiler:Languages\Dutch.isl"
Name: "finnish"; MessagesFile: "compiler:Languages\Finnish.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "hebrew"; MessagesFile: "compiler:Languages\Hebrew.isl"
Name: "icelandic"; MessagesFile: "compiler:Languages\Icelandic.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "norwegian"; MessagesFile: "compiler:Languages\Norwegian.isl"
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "slovak"; MessagesFile: "compiler:Languages\Slovak.isl"
Name: "slovenian"; MessagesFile: "compiler:Languages\Slovenian.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "ukrainian"; MessagesFile: "compiler:Languages\Ukrainian.isl"

[CustomMessages]
RemoveDataFolder=Remove all data and configuration?
english.RemoveDataFolder=Remove all data and configuration?
armenian.RemoveDataFolder=Հեռացնե՞լ բոլոր տվյալները և կոնֆիգուրացիան:
brazilianportuguese.RemoveDataFolder=Remover todos os dados e configuração?
bulgarian.RemoveDataFolder=Премахване на всички данни и конфигурация?
catalan.RemoveDataFolder=Vols suprimir totes les dades i la configuració?
corsican.RemoveDataFolder=Eliminate tutti i dati è a cunfigurazione?
czech.RemoveDataFolder=Odebrat všechna data a konfiguraci?
danish.RemoveDataFolder=Remove all data and configuration?
dutch.RemoveDataFolder=Remove all data and configuration?
finnish.RemoveDataFolder=Poistetaanko kaikki tiedot ja asetukset?
french.RemoveDataFolder=Supprimer toutes les données et la configuration ?
german.RemoveDataFolder=Alle Daten und Konfiguration entfernen?
hebrew.RemoveDataFolder=Remove all data and configuration?
icelandic.RemoveDataFolder=Fjarlægja öll gögn og stillingar?
italian.RemoveDataFolder=Rimuovere tutti i dati e la configurazione?
japanese.RemoveDataFolder=すべてのデータと構成を削除しますか?
norwegian.RemoveDataFolder=Vil du fjerne all data og konfigurasjon?
polish.RemoveDataFolder=Usunąć wszystkie dane i konfigurację?
portuguese.RemoveDataFolder=Remover todos os dados e configuração?
russian.RemoveDataFolder=Удалить все данные и конфигурацию?
slovak.RemoveDataFolder=Chcete odstrániť všetky údaje a konfiguráciu?
slovenian.RemoveDataFolder=Želite odstraniti vse podatke in konfiguracijo?
spanish.RemoveDataFolder=¿Eliminar todos los datos y la configuración?
turkish.RemoveDataFolder=Tüm veriler ve yapılandırma kaldırılsın mı?
ukrainian.RemoveDataFolder=Видалити всі дані та конфігурацію?

[Tasks]
Name: "runapp"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"

[Files]
; NOTE: Don't use "Flags: ignoreversion" on any shared system files
Source: "{#MyAppExeLocation}"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppRoot}LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyAppResBinLocation}ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}ffprobe.exe"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}stremio-runtime.exe"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}server.js"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyAppResBinLocation}avcodec-58.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}avdevice-58.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}avfilter-7.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}avformat-58.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}avutil-56.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}postproc-55.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}swresample-3.dll"; DestDir: "{app}"; Flags: ignoreversion signonce
Source: "{#MyAppResBinLocation}swscale-5.dll"; DestDir: "{app}"; Flags: ignoreversion signonce

[Registry]

; stremio: protocol
Root: HKA; Subkey: "Software\Classes\StremioService"; ValueType: string; ValueName: ""; ValueData: "URL:Stremio Protocol"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\StremioService"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\StremioService\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\StremioService\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""-o"" ""%1"""; Flags: uninsdeletekey

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

; This is used if the desktop shortcut is created by the [run] section.
; [UninstallDelete]
; Type: files; Name: "{autodesktop}\{#MyAppName}.lnk"

; [Run]
; Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
; Filename: "cmd"; Parameters: "/c copy ""{autoprograms}\{#MyAppName}.lnk"" ""{autodesktop}"""; Description: "{cm:CreateDesktopIcon}"; Flags: postinstall skipifsilent shellexec runhidden waituntilterminated runascurrentuser
