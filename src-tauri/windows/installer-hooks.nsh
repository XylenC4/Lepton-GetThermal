; Lepton-GetThermal NSIS hooks: install the Visual C++ 2015-2022 x64 runtime
; (needed by the statically linked ONNX Runtime) if it is not present yet.
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
  ${If} $0 != 1
    DetailPrint "Installing Microsoft Visual C++ runtime..."
    ExecWait '"$INSTDIR\vc_redist.x64.exe" /install /quiet /norestart' $1
    DetailPrint "VC++ runtime installer exit code: $1"
  ${EndIf}
  Delete "$INSTDIR\vc_redist.x64.exe"
!macroend
