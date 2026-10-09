; Instalador de TransFEL para Windows (Inno Setup 6)
; Compilar:  ISCC.exe /DMiVersion="v0.1.0" installer\transfel.iss

#ifndef MiVersion
  #define MiVersion "v0.1.1"
#endif

#define MiNombre "TransFEL"
#define MiAutor "Felipe Villaquiran"
#define MiUrl "https://github.com/pipeeex/TransFEL"
#define MiEjecutable "TransFEL.exe"

[Setup]
AppId={{7F3A9C21-5E84-4C6B-9A1D-TRANSFEL0001}
AppName={#MiNombre}
AppVersion={#MiVersion}
AppPublisher={#MiAutor}
AppPublisherURL={#MiUrl}
AppSupportURL={#MiUrl}/issues
DefaultDirName={autopf}\{#MiNombre}
DefaultGroupName={#MiNombre}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=salida
OutputBaseFilename=TransFEL-Setup-{#MiVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Icono del propio instalador y de Agregar o quitar programas
SetupIconFile=..\packaging\icono\transfel.ico
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\{#MiEjecutable}

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; Ejecutable principal
Source: "..\dist\{#MiEjecutable}"; DestDir: "{app}"; Flags: ignoreversion

; ADB y FFmpeg empaquetados: no tocan el PATH del sistema
Source: "..\dist\tools\*"; DestDir: "{app}\tools"; Flags: ignoreversion recursesubdirs

; Licencias y documentacion
Source: "..\dist\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\{#MiNombre}"; Filename: "{app}\{#MiEjecutable}"
Name: "{group}\Desinstalar {#MiNombre}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MiNombre}"; Filename: "{app}\{#MiEjecutable}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MiEjecutable}"; Description: "{cm:LaunchProgram,{#MiNombre}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Cierra el servidor de ADB antes de borrar los binarios, o quedan bloqueados.
Filename: "{app}\tools\adb.exe"; Parameters: "kill-server"; Flags: runhidden skipifdoesntexist

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\TransFEL"
Type: filesandordirs; Name: "{userappdata}\TransFEL"
