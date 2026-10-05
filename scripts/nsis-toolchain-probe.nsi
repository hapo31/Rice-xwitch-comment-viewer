Unicode true
!if ${NSIS_PACKEDVERSION} != 0x03011000
  !error "NSIS compiler packed version differs from the reviewed 3.11 release"
!endif
!include "MUI2.nsh"
!include "Win\RestartManager.nsh"
!include "nsDialogs.nsh"
Name "Rice NSIS toolchain probe"
OutFile "/tmp/rice-nsis-toolchain-probe.exe"
Section
  System::Call 'kernel32::GetCurrentProcessId() i.r0'
  DetailPrint "Matching native compiler and Windows stub/plugins: $0"
SectionEnd
