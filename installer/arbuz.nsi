Unicode true
SetCompressor /SOLID lzma
ManifestDPIAware true

!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef ROOT
  !define ROOT ".."
!endif
!ifndef APP_VERSION
  !define APP_VERSION "1.0.0"
!endif
!ifndef APP_VER4
  !define APP_VER4 "1.0.0.0"
!endif

!define APP_NAME "Arbuz Launcher"
!define APP_EXE "arbuz-launcher.exe"
!define APP_PUBLISHER "Arbuz"
!define APP_DATA "$PROFILE\arbuz-launcher"
!define APP_DATA_BESIDE "$INSTDIR\arbuz-launcher"
!define APP_DATA_OLD "$APPDATA\Arbuz Launcher"
!define APP_KEY "Software\${APP_PUBLISHER}\${APP_NAME}"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"

Name "${APP_NAME}"
OutFile "${ROOT}\src-tauri\target\release\bundle\arbuz\Arbuz-Launcher-${APP_VERSION}-setup.exe"
InstallDir "$PROFILE\arbuz-launcher"
InstallDirRegKey HKCU "${APP_KEY}" "InstallDir"
RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${APP_VER4}"
VIAddVersionKey /LANG=1033 "ProductName" "${APP_NAME}"
VIAddVersionKey /LANG=1033 "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "${APP_PUBLISHER}"

!define MUI_ICON "icon.ico"
!define MUI_UNICON "icon.ico"
!define MUI_BGCOLOR "11140F"
!define MUI_TEXTCOLOR "E1E4DA"
!define MUI_INSTFILESPAGE_COLORS "E1E4DA 11140F"
!define MUI_INSTFILESPAGE_PROGRESSBAR "colored"
!define MUI_ABORTWARNING
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${APP_KEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Language"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "$(RunApp)"
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(Desktop)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Russian"
!insertmacro MUI_LANGUAGE "Polish"
!insertmacro MUI_LANGUAGE "Ukrainian"
!insertmacro MUI_LANGUAGE "Belarusian"
!insertmacro MUI_LANGUAGE "Serbian"
!insertmacro MUI_LANGUAGE "Slovak"
!insertmacro MUI_LANGUAGE "Esperanto"
!insertmacro MUI_LANGUAGE "Estonian"
!insertmacro MUI_LANGUAGE "Finnish"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "Latvian"
!insertmacro MUI_LANGUAGE "Lithuanian"
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "Japanese"
!insertmacro MUI_LANGUAGE "Uzbek"
!insertmacro MUI_LANGUAGE "Georgian"
!insertmacro MUI_LANGUAGE "Kazakh"
!insertmacro MUI_LANGUAGE "Kyrgyz"
!insertmacro MUI_LANGUAGE "Tajik"
!insertmacro MUI_LANGUAGE "Inuktitut"
!insertmacro MUI_LANGUAGE "Olbanian"
!insertmacro MUI_RESERVEFILE_LANGDLL

!include "lang\custom.nsh"
!include "lang\strings.nsh"

LangString RunApp ${LANG_ENGLISH} "Run ${APP_NAME}"
LangString RunApp ${LANG_RUSSIAN} "Запустить ${APP_NAME}"
LangString Desktop ${LANG_ENGLISH} "Create desktop shortcut"
LangString Desktop ${LANG_RUSSIAN} "Создать ярлык на рабочем столе"
LangString AskData ${LANG_ENGLISH} "Also delete settings, profiles, worlds and accounts?"
LangString AskData ${LANG_RUSSIAN} "Удалить также настройки, профили, миры и аккаунты?"
LangString Busy ${LANG_ENGLISH} "The launcher is running. Close it and try again."
LangString Busy ${LANG_RUSSIAN} "Лаунчер запущен. Закройте его и повторите."

Function .onInit
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Function un.onInit
  !insertmacro MUI_UNGETLANGUAGE
FunctionEnd

Section "Install"
  SetOutPath $INSTDIR

  IfFileExists "$INSTDIR\${APP_EXE}" 0 files_free
    Delete "$INSTDIR\${APP_EXE}"
    IfFileExists "$INSTDIR\${APP_EXE}" 0 files_free
      MessageBox MB_ICONSTOP "$(Busy)"
      Abort
  files_free:

  File "/oname=${APP_EXE}" "${ROOT}\src-tauri\target\release\${APP_EXE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  WriteRegStr HKCU "${APP_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Function CreateDesktopShortcut
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
FunctionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "${APP_KEY}"
  DeleteRegKey /ifempty HKCU "Software\${APP_PUBLISHER}"

  IfSilent keep
  MessageBox MB_YESNO|MB_ICONQUESTION "$(AskData)" IDNO keep
    ; Данные лежат в папке лаунчера или в отдельной папке рядом с ним.
    RMDir /r "${APP_DATA_BESIDE}"
    RMDir /r "${APP_DATA}"
    RMDir /r "${APP_DATA_OLD}"
    StrCmp "$INSTDIR" "${APP_DATA}" 0 keep
      RMDir /r "$INSTDIR"
  keep:
SectionEnd
