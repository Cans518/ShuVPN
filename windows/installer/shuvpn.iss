#ifndef BundleDir
  #error BundleDir is required
#endif
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif

[Setup]
AppId=work.shuvpn.app
AppName=ShuVPN
AppVersion={#AppVersion}
AppPublisher=ShuVPN
DefaultDirName={localappdata}\Programs\ShuVPN
DefaultGroupName=ShuVPN
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=ShuVPN-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
AllowNoIcons=yes
UninstallDisplayIcon={app}\shuvpn.exe
AppMutex=ShuVPN.Desktop.Running,Global\ShuVPN.Desktop.Running
SetupMutex=ShuVPN.Desktop.Setup,Global\ShuVPN.Desktop.Setup
CloseApplications=no
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimp"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "{#BundleDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "package-manager.txt"; DestDir: "{app}"; DestName: ".package-manager"; Flags: ignoreversion

[Icons]
Name: "{group}\ShuVPN"; Filename: "{app}\shuvpn.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\ShuVPN"; Filename: "{app}\shuvpn.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Code]
var
  TransactionHandle: NativeInt;

function CreateTransactionMutex(Attributes: NativeInt; InitialOwner: Integer; Name: String): NativeInt;
  external 'CreateMutexW@kernel32.dll stdcall';

function VerificationReserved: Boolean;
begin
  Result := CheckForMutexes('Global\ShuVPN.Desktop.InstallerSmoke') and
            (ExpandConstant('{param:AUTOMATION|0}') <> '1');
end;

function BeginInstallation: Boolean;
begin
  if TransactionHandle = 0 then begin
    TransactionHandle := CreateTransactionMutex(0, 0, 'Global\ShuVPN.Desktop.Transaction');
    if (TransactionHandle = 0) or (DLLGetLastError = 183) then begin
      SuppressibleMsgBox('Another ShuVPN installation or removal is in progress.', mbError, MB_OK, IDOK);
      Result := False;
      exit;
    end;
  end;
  if VerificationReserved then begin
    SuppressibleMsgBox('Installation verification is in progress. Try again after it finishes.', mbError, MB_OK, IDOK);
    Result := False;
    exit;
  end;
  CreateMutex('ShuVPN.Desktop.Installing');
  CreateMutex('Global\ShuVPN.Desktop.Installing');
  Result := not CheckForMutexes('ShuVPN.Desktop.Running,Global\ShuVPN.Desktop.Running');
  if not Result then
    SuppressibleMsgBox('Close ShuVPN before installing or removing it.', mbError, MB_OK, IDOK);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if VerificationReserved then
    Result := 'Installation verification is in progress. Try again after it finishes.';
end;

function InitializeSetup: Boolean;
begin
  Result := BeginInstallation;
end;

function InitializeUninstall: Boolean;
begin
  Result := BeginInstallation;
end;
