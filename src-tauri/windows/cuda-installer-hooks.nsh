; Force CUDA variant to install into the same directory as the standard build
; so desktop/Start Menu shortcuts always point to the correct exe after reinstall.
!macro NSIS_HOOK_PREINSTALL
  StrCpy $INSTDIR "$LOCALAPPDATA\NexusVoice"
!macroend
