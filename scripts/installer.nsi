; Scribe Windows installer — NSIS 3.x, per-user install (no admin rights).
; Build with: makensis -DAPP_VERSION=1.0.8 scripts\installer.nsi
; Expects target\release\scribe.exe to exist (cargo build --release --workspace --locked).

!define APP_NAME "Scribe"
!define APP_PUBLISHER "KelpHect"
!define APP_EXE "scribe.exe"
!ifndef APP_VERSION
  !define APP_VERSION "0.0.0"
!endif

!include "MUI2.nsh"

Name "${APP_NAME}"
OutFile "..\bin\ScribeSetup-${APP_VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ManifestDPIAware true

!define MUI_ICON "..\assets\scribe-icon-v2.ico"
!define MUI_UNICON "..\assets\scribe-icon-v2.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  ; NSIS shows a built-in retry dialog if scribe.exe is locked by a running
  ; instance; close Scribe before upgrading.
  File "..\target\release\${APP_EXE}"
  File "..\assets\scribe-icon-v2.ico"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\scribe-icon-v2.ico"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\Uninstall ${APP_NAME}.lnk" "$INSTDIR\Uninstall.exe"

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Per-user Add/Remove Programs entry (HKCU, no elevation).
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayIcon" "$INSTDIR\scribe-icon-v2.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  ; Only Scribe-owned program files are removed; user data under
  ; %APPDATA%\Scribe (settings.toml, scribe.redb) is preserved.
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\scribe-icon-v2.ico"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Uninstall ${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd
