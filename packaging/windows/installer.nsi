; The Windows installer (component 25, piece C): per user, no admin. Puts fleet-native.exe, the README and Fleet's server
; (`fleet\`) in %LOCALAPPDATA%\Programs\Fleet-Native, adds a Start-menu entry and an entry in Settings > Apps, then starts
; the app. One click, a progress bar, done: the shape of Fleet's own electron-builder NSIS installer. Running it again
; installs over the old copy (the server folder is replaced whole, so no file of an older server stays).
; The uninstaller removes what this installer put there and nothing else: the data folder (~\.weave) is never named.
;
; Built by scripts/make-installer.ps1, which passes:
;   /DSTAGE=<folder holding fleet-native.exe, README.md, fleet\>  /DBUILD=<build number>  /DOUTFILE=<setup.exe>  /DICON=<.ico>
; Silent: `setup.exe /S [/D=<folder>]`; exit code 2 when a copy of the app is running there (nothing is changed).

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!ifndef STAGE | BUILD | OUTFILE | ICON
  !error "make-installer.ps1 passes STAGE, BUILD, OUTFILE and ICON"
!endif

!define PRODUCT "Fleet Native"
!define VERSION "0.0.${BUILD}"
!define EXE "fleet-native.exe"
!define UNINSTALLER "Uninstall Fleet Native.exe"
!define SHORTCUT "$SMPROGRAMS\${PRODUCT}.lnk"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\FleetNative"

!include "FileFunc.nsh"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
Icon "${ICON}"
UninstallIcon "${ICON}"
; Beside Fleet's own (%LOCALAPPDATA%\Programs\Fleet), so both can be installed.
InstallDir "$LOCALAPPDATA\Programs\Fleet-Native"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
BrandingText "${PRODUCT} ${VERSION}"
Caption "${PRODUCT} Setup"
UninstallCaption "${PRODUCT} Uninstall"
ShowInstDetails nevershow
ShowUninstDetails nevershow
AutoCloseWindow true

VIProductVersion "0.0.${BUILD}.0"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "CompanyName" "Weave"
VIAddVersionKey "FileDescription" "${PRODUCT} Setup"
VIAddVersionKey "FileVersion" "0.0.${BUILD}.0"
VIAddVersionKey "ProductVersion" "0.0.${BUILD}.0"
VIAddVersionKey "LegalCopyright" "Weave"

; One click: no welcome, licence or folder page, only the progress.
Page instfiles
UninstPage instfiles

; A running app holds its exe (and its Fleet holds the server's): Windows won't let either be written. Say so and wait for a
; Retry; silent, give up with exit code 2 before anything is touched.
!macro WAIT_UNTIL_CLOSED
  retry:
  ClearErrors
  IfFileExists "$INSTDIR\${EXE}" 0 +4
    FileOpen $0 "$INSTDIR\${EXE}" a
    IfErrors running
    FileClose $0
  IfFileExists "$INSTDIR\fleet\WeaveFleet.Api.exe" 0 free
    FileOpen $0 "$INSTDIR\fleet\WeaveFleet.Api.exe" a
    IfErrors running
    FileClose $0
    Goto free
  running:
  IfSilent 0 +3
    SetErrorLevel 2
    Quit
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "${PRODUCT} is running. Close it, then press Retry." IDRETRY retry
  SetErrorLevel 2
  Quit
  free:
!macroend

Section "Install"
  !insertmacro WAIT_UNTIL_CLOSED
  SetOutPath "$INSTDIR"
  RMDir /r "$INSTDIR\fleet"
  File /r "${STAGE}\*"
  WriteUninstaller "$INSTDIR\${UNINSTALLER}"
  CreateShortCut "${SHORTCUT}" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0 SW_SHOWNORMAL "" "Run and watch many AI coding agents at once"

  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Weave"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\${UNINSTALLER}"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINSTALLER}" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

; As Fleet's one-click installer does: the app starts when the install is done (not from a silent install).
Function .onInstSuccess
  IfSilent +2
    Exec '"$INSTDIR\${EXE}"'
FunctionEnd

Section "Uninstall"
  !insertmacro WAIT_UNTIL_CLOSED
  Delete "${SHORTCUT}"
  RMDir /r "$INSTDIR\fleet"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\${UNINSTALLER}"
  ; Only when nothing else is left in it: a file the user put there keeps the folder.
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINST_KEY}"
SectionEnd
