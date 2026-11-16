; Lepton-GetThermal NSIS hooks: install the Visual C++ 2015-2022 runtime (x64 or arm64,
; whichever vc_redist.exe the build staged) if it is not present yet. It is needed by the
; statically linked ONNX Runtime.
!include x64.nsh
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  ${If} ${IsNativeARM64}
    ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\arm64" "Installed"
  ${Else}
    ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
  ${EndIf}
  ${If} $0 != 1
    DetailPrint "Installing Microsoft Visual C++ runtime..."
    ExecWait '"$INSTDIR\vc_redist.exe" /install /quiet /norestart' $1
    DetailPrint "VC++ runtime installer exit code: $1"
  ${EndIf}
  Delete "$INSTDIR\vc_redist.exe"
!macroend
