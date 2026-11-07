; Aztek installer, for Inno Setup 6. Built by packaging\windows\build-installer.ps1 from what build.ps1 leaves in
; build\dist. Pass the version with iscc /DAppVersion=0.1.0. Not signed: Windows SmartScreen warns about it.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

[Setup]
AppId={{6B1F2E3A-7C54-4D0E-9A21-3F5C8E7D1A42}
AppName=Aztek
AppVersion={#AppVersion}
AppVerName=Aztek {#AppVersion}
AppPublisher=Simon Legrand
AppPublisherURL=https://github.com/srlegrand/AKA-Crest-Aztek
DefaultDirName={autopf}\Aztek
DefaultGroupName=Aztek
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=aztek-{#AppVersion}-setup-x64
SetupIconFile=..\..\app\icons\aztek.ico
UninstallDisplayIcon={app}\aztek.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
Name: "associate"; Description: "Open .ma files with Aztek"; GroupDescription: "File associations:"; Flags: unchecked

[Files]
Source: "..\..\build\dist\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Aztek"; Filename: "{app}\aztek.exe"
Name: "{autodesktop}\Aztek"; Filename: "{app}\aztek.exe"; Tasks: desktopicon

[Registry]
; {app}\aztek.exe also stays in "Open with" for .ma after the association is taken over by something else.
Root: HKA; Subkey: "Software\Classes\.ma"; ValueType: string; ValueName: ""; ValueData: "Aztek.Scene"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.ma\OpenWithProgids"; ValueType: string; ValueName: "Aztek.Scene"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Aztek.Scene"; ValueType: string; ValueName: ""; ValueData: "ASCII scene"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Aztek.Scene\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\aztek.exe,0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Aztek.Scene\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\aztek.exe"" ""%1"""; Tasks: associate

[Run]
Filename: "{app}\aztek.exe"; Description: "Start Aztek"; Flags: nowait postinstall skipifsilent
