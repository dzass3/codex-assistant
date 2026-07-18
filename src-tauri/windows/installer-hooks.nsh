; Tauri loads this hook through bundle.windows.nsis.installerHooks. Keep the
; migration deliberately narrow: it acts only on the exact 0.4 per-user NSIS
; identity after cross-checking its registry metadata and installed files.
!define LEGACY_PRODUCT_NAME "Codex Agent Monitor"
!define LEGACY_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LEGACY_PRODUCT_NAME}"
!define LEGACY_MANU_PRODUCT_KEY "Software\codexagentmonitor\${LEGACY_PRODUCT_NAME}"
!define LEGACY_MAIN_BINARY "codex-agent-model-monitor.exe"

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Checking for a verified Codex Agent Monitor 0.4 installation"

  ; Tauri 0.4 wrote all four values below in HKCU. Do nothing if any identity
  ; proof is absent or inconsistent, so an unrelated matching registry key is
  ; never removed or used as an installation target.
  ReadRegStr $R0 HKCU "${LEGACY_UNINST_KEY}" "DisplayName"
  StrCmp $R0 "${LEGACY_PRODUCT_NAME}" 0 legacy_migration_done
  ReadRegStr $R1 HKCU "${LEGACY_UNINST_KEY}" "InstallLocation"
  ReadRegStr $R2 HKCU "${LEGACY_UNINST_KEY}" "UninstallString"
  ReadRegStr $R3 HKCU "${LEGACY_UNINST_KEY}" "MainBinaryName"
  ReadRegStr $R4 HKCU "${LEGACY_MANU_PRODUCT_KEY}" ""
  StrCmp $R1 "" legacy_migration_done
  StrCmp $R2 "" legacy_migration_done
  StrCmp $R3 "${LEGACY_MAIN_BINARY}" 0 legacy_migration_done
  StrCmp $R4 "" legacy_migration_done

  ; InstallLocation and UninstallString are quoted by Tauri's NSIS template;
  ; compare them to values derived from its own persisted install directory.
  StrCpy $R5 "$\"$R4$\""
  StrCmp $R1 $R5 0 legacy_migration_done
  StrCpy $R5 "$\"$R4\uninstall.exe$\""
  StrCmp $R2 $R5 0 legacy_migration_done
  ${IfNot} ${FileExists} "$R4\${LEGACY_MAIN_BINARY}"
    Goto legacy_migration_done
  ${EndIf}
  ${IfNot} ${FileExists} "$R4\uninstall.exe"
    Goto legacy_migration_done
  ${EndIf}

  ; The old uninstaller removes only its known files. /UPDATE leaves shortcuts
  ; alone, so this hook can verify each legacy target before deleting it.
  DetailPrint "Migrating verified Codex Agent Monitor installation"
  ClearErrors
  ExecWait '$R2 /S /UPDATE _?=$R4' $0
  ${If} $0 <> 0
    Abort "The verified Codex Agent Monitor installation could not be removed. Codex Assistant was not installed."
  ${EndIf}
  ; NSIS cannot always remove its own running uninstaller. Its exact path was
  ; validated above, so remove that one leftover file after the process exits.
  Delete "$R4\uninstall.exe"
  ${If} ${FileExists} "$R4\${LEGACY_MAIN_BINARY}"
  ${OrIf} ${FileExists} "$R4\uninstall.exe"
    Abort "The verified Codex Agent Monitor installation could not be removed. Codex Assistant was not installed."
  ${EndIf}

  ; Remove only the exact old shortcuts whose target is the verified legacy
  ; executable. Same-named shortcuts targeting any other program are retained.
  !insertmacro IsShortcutTarget "$SMPROGRAMS\${LEGACY_PRODUCT_NAME}.lnk" "$R4\${LEGACY_MAIN_BINARY}"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$SMPROGRAMS\${LEGACY_PRODUCT_NAME}.lnk"
    Delete "$SMPROGRAMS\${LEGACY_PRODUCT_NAME}.lnk"
  ${EndIf}
  !insertmacro IsShortcutTarget "$DESKTOP\${LEGACY_PRODUCT_NAME}.lnk" "$R4\${LEGACY_MAIN_BINARY}"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$DESKTOP\${LEGACY_PRODUCT_NAME}.lnk"
    Delete "$DESKTOP\${LEGACY_PRODUCT_NAME}.lnk"
  ${EndIf}

  ; Install Codex Assistant in the proven legacy location. The app-owned
  ; codex-agent-monitor settings directory is outside $INSTDIR and untouched.
  StrCpy $INSTDIR "$R4"
  ; Tauri calls SetOutPath before NSIS_HOOK_PREINSTALL. Keep its file-copy
  ; destination aligned with the migrated install directory as well.
  SetOutPath "$INSTDIR"
legacy_migration_done:
!macroend
