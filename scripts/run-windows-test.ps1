param(
    [Parameter(Position = 0, Mandatory = $true)][string]$Binary,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$TestArguments
)
$ErrorActionPreference = 'Stop'
# Cargo may relink a libtest between --no-run and the full test invocation.
# Apply the same production Common Controls v6 requirement immediately before
# each actual execution; do not rely on a previously modified output file.
$sdkBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$mt = Get-ChildItem -LiteralPath $sdkBin -Filter mt.exe -Recurse -File |
    Where-Object { $_.DirectoryName -match '\\x64$' } |
    Sort-Object FullName | Select-Object -Last 1
if (-not $mt) { throw 'Windows SDK manifest tool is required' }
$manifest = Join-Path $PSScriptRoot '../src-tauri/tests/native-windows.manifest'
& $mt.FullName -manifest $manifest "-outputresource:$Binary;#1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $Binary @TestArguments
exit $LASTEXITCODE
