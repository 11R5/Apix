; The Windows installer (Inno Setup 6). Built by scripts/package-windows.ps1:
;   iscc /DAppVersion=1.2.3 /DBinDir=<folder with the exes> /DOutDir=dist packaging\windows\apix.iss
; It installs for the current user without asking for admin rights, or for
; everyone when started as admin, and can put `apix` on the PATH.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef BinDir
  #define BinDir "..\..\target\release"
#endif
#ifndef OutDir
  #define OutDir "..\..\dist"
#endif

[Setup]
AppId={{6B0E2B9C-4F1D-4E43-9A57-2C1F0D9A8E31}
AppName=Apix
AppVersion={#AppVersion}
AppVerName=Apix {#AppVersion}
AppPublisher=Ahmet Demir
AppPublisherURL=https://github.com/11R5/Apix
AppSupportURL=https://github.com/11R5/Apix/issues
AppUpdatesURL=https://github.com/11R5/Apix/releases
DefaultDirName={autopf}\Apix
DefaultGroupName=Apix
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir={#OutDir}
OutputBaseFilename=Apix-{#AppVersion}-windows-x64-setup
SetupIconFile=apix.ico
UninstallDisplayIcon={app}\apix-desktop.exe
UninstallDisplayName=Apix
LicenseFile=..\..\LICENSE
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
ChangesAssociations=yes
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[CustomMessages]
english.AddToPath=Add the apix command line to PATH
turkish.AddToPath=apix komut satırını PATH'e ekle
german.AddToPath=Die Kommandozeile apix zu PATH hinzufügen
french.AddToPath=Ajouter la ligne de commande apix au PATH
spanish.AddToPath=Añadir la línea de comandos apix al PATH
russian.AddToPath=Добавить командную строку apix в PATH
japanese.AddToPath=apix コマンドラインを PATH に追加
english.Associate=Open .apix request files with Apix
turkish.Associate=.apix istek dosyalarını Apix ile aç
german.Associate=.apix-Anfragedateien mit Apix öffnen
french.Associate=Ouvrir les fichiers de requête .apix avec Apix
spanish.Associate=Abrir los archivos de petición .apix con Apix
russian.Associate=Открывать файлы запросов .apix в Apix
japanese.Associate=.apix リクエストファイルを Apix で開く

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "addtopath"; Description: "{cm:AddToPath}"
Name: "associate"; Description: "{cm:Associate}"

[Files]
Source: "{#BinDir}\apix-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BinDir}\apix.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Apix"; Filename: "{app}\apix-desktop.exe"
Name: "{autodesktop}\Apix"; Filename: "{app}\apix-desktop.exe"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.apix"; ValueType: string; ValueName: ""; ValueData: "Apix.Request"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Apix.Request"; ValueType: string; ValueName: ""; ValueData: "Apix request"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Apix.Request\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\apix-desktop.exe,0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Apix.Request\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\apix-desktop.exe"" ""%1"""; Tasks: associate

[Run]
Filename: "{app}\apix-desktop.exe"; Description: "{cm:LaunchProgram,Apix}"; Flags: nowait postinstall skipifsilent

[Code]
// PATH lives in the user's environment for a per-user install, in the
// machine's for an install for everyone.
function PathRoot: Integer;
begin
  if IsAdminInstallMode then Result := HKEY_LOCAL_MACHINE else Result := HKEY_CURRENT_USER;
end;

function PathKey: String;
begin
  if IsAdminInstallMode then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

function HasDir(Paths, Dir: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0;
end;

procedure AddToPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(PathRoot, PathKey, 'Path', Paths) then Paths := '';
  if HasDir(Paths, Dir) then exit;
  if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then Paths := Paths + ';';
  RegWriteExpandStringValue(PathRoot, PathKey, 'Path', Paths + Dir);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths: String;
  P: Integer;
begin
  if not RegQueryStringValue(PathRoot, PathKey, 'Path', Paths) then exit;
  Paths := ';' + Paths + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Paths));
  if P = 0 then exit;
  Delete(Paths, P, Length(Dir) + 1);
  // Drop the separators added above.
  Paths := Copy(Paths, 2, Length(Paths) - 2);
  RegWriteExpandStringValue(PathRoot, PathKey, 'Path', Paths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
