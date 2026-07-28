; Tauri loads these hooks through bundle.windows.nsis.installerHooks. The
; migration is deliberately two-phase: PREINSTALL arms a verified recovery
; path without removing 0.4, and POSTINSTALL cleans up only after proving that
; the complete 0.5 identity exists in the legacy directory.
!define LEGACY_PRODUCT_NAME "Codex Agent Monitor"
!define LEGACY_VERSION "0.4.0"
!define LEGACY_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LEGACY_PRODUCT_NAME}"
!define LEGACY_MANU_PRODUCT_KEY "Software\codexagentmonitor\${LEGACY_PRODUCT_NAME}"
!define LEGACY_MAIN_BINARY "codex-agent-model-monitor.exe"
!define LEGACY_UNINSTALL_BACKUP "codex-agent-monitor-0.4.0-uninstall.exe"
!define RETIRED_THEMED_CODEX_SHORTCUT "Codex（主题版）.lnk"

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Checking for a verified Codex Agent Monitor 0.4.0 installation"

  ; Tauri 0.4 wrote these values in HKCU. Do nothing unless every identity
  ; proof is present and consistent, including the exact legacy version.
  ReadRegStr $R0 HKCU "${LEGACY_UNINST_KEY}" "DisplayName"
  StrCmp $R0 "${LEGACY_PRODUCT_NAME}" 0 legacy_pre_done
  ReadRegStr $R1 HKCU "${LEGACY_UNINST_KEY}" "InstallLocation"
  ReadRegStr $R2 HKCU "${LEGACY_UNINST_KEY}" "UninstallString"
  ReadRegStr $R3 HKCU "${LEGACY_UNINST_KEY}" "MainBinaryName"
  ReadRegStr $R4 HKCU "${LEGACY_MANU_PRODUCT_KEY}" ""
  ReadRegStr $R6 HKCU "${LEGACY_UNINST_KEY}" "DisplayVersion"
  StrCmp $R1 "" legacy_pre_done
  StrCmp $R2 "" legacy_pre_done
  StrCmp $R3 "${LEGACY_MAIN_BINARY}" 0 legacy_pre_done
  StrCmp $R4 "" legacy_pre_done
  StrCmp $R6 "${LEGACY_VERSION}" 0 legacy_pre_done

  ; InstallLocation is quoted by Tauri's NSIS template. Cross-check it against
  ; the independently persisted manufacturer/product installation directory.
  StrCpy $R5 "$\"$R4$\""
  StrCmp $R1 $R5 0 legacy_pre_done
  ${IfNot} ${FileExists} "$R4\${LEGACY_MAIN_BINARY}"
    Goto legacy_pre_done
  ${EndIf}

  ; A first migration points at the original uninstaller. A retry after an
  ; interrupted install points at the exact recovery copy created below.
  StrCpy $R5 "$\"$R4\uninstall.exe$\""
  StrCpy $R7 "$\"$R4\${LEGACY_UNINSTALL_BACKUP}$\""
  ${If} $R2 == $R5
    ${IfNot} ${FileExists} "$R4\uninstall.exe"
      Goto legacy_pre_done
    ${EndIf}
    ; An unexpected pre-existing recovery filename is ambiguous. Leave the
    ; legacy installation untouched rather than overwriting an unknown file.
    ${If} ${FileExists} "$R4\${LEGACY_UNINSTALL_BACKUP}"
      Goto legacy_pre_done
    ${EndIf}
    !insertmacro CheckIfAppIsRunning "${LEGACY_MAIN_BINARY}" "${LEGACY_PRODUCT_NAME}"
    ClearErrors
    CopyFiles /SILENT "$R4\uninstall.exe" "$R4\${LEGACY_UNINSTALL_BACKUP}"
    ${If} ${Errors}
      Delete "$R4\${LEGACY_UNINSTALL_BACKUP}"
      Abort "Codex Agent Monitor recovery could not be prepared. Codex Assistant was not installed."
    ${EndIf}
    ${IfNot} ${FileExists} "$R4\${LEGACY_UNINSTALL_BACKUP}"
      Abort "Codex Agent Monitor recovery could not be prepared. Codex Assistant was not installed."
    ${EndIf}
    WriteRegStr HKCU "${LEGACY_UNINST_KEY}" "UninstallString" "$\"$R4\${LEGACY_UNINSTALL_BACKUP}$\""
    ReadRegStr $R2 HKCU "${LEGACY_UNINST_KEY}" "UninstallString"
    StrCmp $R2 $R7 legacy_pre_armed 0
    Delete "$R4\${LEGACY_UNINSTALL_BACKUP}"
    Abort "Codex Agent Monitor recovery could not be registered. Codex Assistant was not installed."
  ${ElseIf} $R2 == $R7
    ${IfNot} ${FileExists} "$R4\${LEGACY_UNINSTALL_BACKUP}"
      Goto legacy_pre_done
    ${EndIf}
    !insertmacro CheckIfAppIsRunning "${LEGACY_MAIN_BINARY}" "${LEGACY_PRODUCT_NAME}"
  ${Else}
    Goto legacy_pre_done
  ${EndIf}

legacy_pre_armed:
  ; Keep the working 0.4 executable, recovery uninstaller, registry, and old
  ; shortcuts until POSTINSTALL proves the new installation is complete.
  StrCpy $INSTDIR "$R4"
  ; Tauri calls SetOutPath before NSIS_HOOK_PREINSTALL, so synchronize its
  ; file-copy destination after adopting the verified legacy directory.
  SetOutPath "$INSTDIR"
legacy_pre_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Re-validate the staged legacy identity; registers used during installation
  ; are scratch state and are intentionally not trusted across hook phases.
  ReadRegStr $R0 HKCU "${LEGACY_UNINST_KEY}" "DisplayName"
  StrCmp $R0 "${LEGACY_PRODUCT_NAME}" 0 legacy_post_done
  ReadRegStr $R1 HKCU "${LEGACY_UNINST_KEY}" "InstallLocation"
  ReadRegStr $R2 HKCU "${LEGACY_UNINST_KEY}" "UninstallString"
  ReadRegStr $R3 HKCU "${LEGACY_UNINST_KEY}" "MainBinaryName"
  ReadRegStr $R4 HKCU "${LEGACY_MANU_PRODUCT_KEY}" ""
  ReadRegStr $R6 HKCU "${LEGACY_UNINST_KEY}" "DisplayVersion"
  StrCmp $R3 "${LEGACY_MAIN_BINARY}" 0 legacy_post_done
  StrCmp $R6 "${LEGACY_VERSION}" 0 legacy_post_done
  StrCmp $R4 "" legacy_post_done
  StrCpy $R5 "$\"$R4$\""
  StrCmp $R1 $R5 0 legacy_post_done
  StrCpy $R7 "$\"$R4\${LEGACY_UNINSTALL_BACKUP}$\""
  StrCmp $R2 $R7 0 legacy_post_done
  ${IfNot} ${FileExists} "$R4\${LEGACY_MAIN_BINARY}"
    Goto legacy_post_done
  ${EndIf}
  ${IfNot} ${FileExists} "$R4\${LEGACY_UNINSTALL_BACKUP}"
    Goto legacy_post_done
  ${EndIf}

  ; Prove the full 0.5 registry and on-disk identity before deleting any 0.4
  ; recovery material. A directory masquerading as either file fails closed.
  StrCmp $INSTDIR $R4 0 legacy_post_rollback
  ReadRegStr $0 HKCU "${UNINSTKEY}" "DisplayName"
  ReadRegStr $1 HKCU "${UNINSTKEY}" "DisplayVersion"
  ReadRegStr $2 HKCU "${UNINSTKEY}" "InstallLocation"
  ReadRegStr $3 HKCU "${UNINSTKEY}" "UninstallString"
  ReadRegStr $4 HKCU "${UNINSTKEY}" "MainBinaryName"
  StrCmp $0 "${PRODUCTNAME}" 0 legacy_post_rollback
  StrCmp $1 "${VERSION}" 0 legacy_post_rollback
  StrCmp $4 "${MAINBINARYNAME}.exe" 0 legacy_post_rollback
  StrCpy $5 "$\"$R4$\""
  StrCmp $2 $5 0 legacy_post_rollback
  StrCpy $6 "$\"$R4\uninstall.exe$\""
  StrCmp $3 $6 0 legacy_post_rollback
  ${IfNot} ${FileExists} "$R4\${MAINBINARYNAME}.exe"
    Goto legacy_post_rollback
  ${EndIf}
  System::Call 'kernel32::GetFileAttributesW(w "$R4\${MAINBINARYNAME}.exe") i .r7'
  IntOp $7 $7 & 0x10
  ${If} $7 <> 0
    Goto legacy_post_rollback
  ${EndIf}
  ${IfNot} ${FileExists} "$R4\uninstall.exe"
    Goto legacy_post_rollback
  ${EndIf}
  System::Call 'kernel32::GetFileAttributesW(w "$R4\uninstall.exe") i .r7'
  IntOp $7 $7 & 0x10
  ${If} $7 <> 0
    Goto legacy_post_rollback
  ${EndIf}

  DetailPrint "Codex Assistant ${VERSION} verified; removing the legacy identity"
  Delete "$R4\${LEGACY_MAIN_BINARY}"
  ${If} ${FileExists} "$R4\${LEGACY_MAIN_BINARY}"
    Abort "Codex Assistant was installed, but the legacy executable could not be removed. Its recovery identity was retained."
  ${EndIf}

  ; Remove only old shortcuts whose target is the now-retired verified binary.
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

  Delete "$R4\${LEGACY_UNINSTALL_BACKUP}"
  ${If} ${FileExists} "$R4\${LEGACY_UNINSTALL_BACKUP}"
    Abort "Codex Assistant was installed, but legacy recovery cleanup did not finish. The legacy registry identity was retained."
  ${EndIf}
  DeleteRegKey HKCU "${LEGACY_UNINST_KEY}"
  DeleteRegKey HKCU "${LEGACY_MANU_PRODUCT_KEY}"
  Goto legacy_post_done

legacy_post_rollback:
  ; No old executable, recovery uninstaller, legacy registry value, or legacy
  ; shortcut has been removed. The Add/Remove Programs rollback entry remains
  ; pointed at the exact recovery uninstaller created in PREINSTALL.
  Abort "Codex Assistant installation could not be verified. Codex Agent Monitor recovery was retained."
legacy_post_done:
  ; Codex Assistant 0.10.0 no longer owns an alternate Codex entry. Remove only
  ; the exact retired shortcut when it still targets this installed binary.
  !insertmacro IsShortcutTarget "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}"
    Delete "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro IsShortcutTarget "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}"
    Delete "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}"
  ${EndIf}
!macroend
