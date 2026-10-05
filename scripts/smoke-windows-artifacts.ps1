param(
    [Parameter(Mandatory = $true)][string]$Artifacts,
    [Parameter(Mandatory = $true)][string]$Commit,
    [string]$Tag = "",
    [string]$Report = "windows-smoke-report.json"
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# This script installs software and probes a real user profile. Never run it
# against a developer's workstation or a self-hosted runner with real Rice data.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or -not $env:RUNNER_TEMP) {
    throw 'Windows artifact smoke requires a fresh GitHub-hosted runner'
}
$scratch = Join-Path $env:RUNNER_TEMP ("rice-artifact-smoke-" + [Guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $scratch
$portable = Join-Path $scratch 'portable'
$installed = Join-Path $scratch 'installed Rice'
$profileData = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'dev.rice.tts'
if (Test-Path -LiteralPath $profileData) { throw 'Pre-existing Rice profile must not be used or deleted' }
function Rice-Registrations {
    $keys = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*')
    foreach ($key in $keys) {
        Get-ItemProperty -Path $key -ErrorAction SilentlyContinue | Where-Object { $_.PSObject.Properties['DisplayName'] -and $_.DisplayName -eq 'Rice' }
    }
}
if (@(Rice-Registrations).Count -ne 0) { throw 'Pre-existing Rice installation must not be uninstalled' }
$manifestText = & node (Join-Path $PSScriptRoot 'verify-release-artifacts.mjs') $Artifacts --commit $Commit --tag $Tag --extract $portable
if ($LASTEXITCODE -ne 0) { throw 'Artifact/ZIP/CRC/source/checksum verification failed' }
$manifest = $manifestText | ConvertFrom-Json
$reportData = [ordered]@{
    schemaVersion = 1; commit = $Commit; tag = $manifest.tag; runId = $env:GITHUB_RUN_ID
    artifactManifestSha256 = (Get-FileHash -LiteralPath (Join-Path $Artifacts 'ARTIFACT-MANIFEST.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    status = 'running'; probes = @()
}
function Probe-App([string]$Executable, [string]$Name) {
    $info = [Diagnostics.FileVersionInfo]::GetVersionInfo($Executable)
    $versionPattern = '^' + [Regex]::Escape($manifest.version) + '(\.0)?$'
    if ($info.ProductVersion -notmatch $versionPattern -or $info.FileMajorPart -ne [int]($manifest.version.Split('.')[0]) -or $info.FileMinorPart -ne [int]($manifest.version.Split('.')[1]) -or $info.FileBuildPart -ne [int]($manifest.version.Split('.')[2]) -or $info.FilePrivatePart -ne 0) { throw "$Name PE product/file version mismatch" }
    $startInfo = [Diagnostics.ProcessStartInfo]::new($Executable)
    $startInfo.UseShellExecute = $false
    $startInfo.WorkingDirectory = Split-Path -Parent $Executable
    $startInfo.Environment['WEBVIEW2_USER_DATA_FOLDER'] = Join-Path $scratch ("webview-" + $Name)
    $process = [Diagnostics.Process]::Start($startInfo)
    $watch = [Diagnostics.Stopwatch]::StartNew()
    try {
        # Both survival and a real visible main window are required. A failed
        # loader/panic is never accepted just because Process.Start succeeded.
        while ($watch.Elapsed.TotalSeconds -lt 30) {
            $process.Refresh()
            if ($process.HasExited) { throw "$Name exited during startup: $($process.ExitCode)" }
            if ($watch.Elapsed.TotalSeconds -ge 5 -and $process.MainWindowHandle -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 100
        }
        if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw "$Name did not show a native window" }
        if (-not $process.CloseMainWindow()) { throw "$Name rejected normal window close" }
        if (-not $process.WaitForExit(15000)) { throw "$Name did not exit normally after window close" }
        if ($process.ExitCode -ne 0) { throw "$Name normal exit failed: $($process.ExitCode)" }
        $script:reportData.probes += [ordered]@{ name = $Name; pid = $process.Id; survivedMs = [int]$watch.Elapsed.TotalMilliseconds; windowShown = $true; exitCode = $process.ExitCode; sha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant() }
    } finally {
        if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
}
function Run-Nsis([string]$Executable, [string]$Arguments, [string]$Name) {
    $process = Start-Process -FilePath $Executable -ArgumentList $Arguments -PassThru
    try {
        if (-not $process.WaitForExit(120000)) { throw "$Name timeout" }
        if ($process.ExitCode -ne 0) { throw "$Name exit code: $($process.ExitCode)" }
        $script:reportData.probes += [ordered]@{ name = $Name; exitCode = $process.ExitCode }
    } finally {
        if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
}
try {
    Probe-App (Join-Path $portable 'rice.exe') 'portable'
    $installer = Join-Path $Artifacts $manifest.installer
    $installerInfo = [Diagnostics.FileVersionInfo]::GetVersionInfo($installer)
    if ($installerInfo.ProductVersion -notmatch ('^' + [Regex]::Escape($manifest.version) + '(\.0)?$')) { throw 'NSIS product version mismatch' }
    # NSIS /D= and _?= must be last and unquoted, including paths with spaces.
    # Never use /NCRC, which would hide installer corruption.
    Run-Nsis $installer "/S /D=$installed" 'silent-install'
    $installedExe = Join-Path $installed 'rice.exe'
    if (-not (Test-Path -LiteralPath $installedExe -PathType Leaf)) { throw 'Installer did not create rice.exe in the isolated directory' }
    if ((Get-FileHash -LiteralPath $installedExe -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $portable 'rice.exe') -Algorithm SHA256).Hash) { throw 'Installed exe differs from the verified portable exe' }
    if ((Get-FileHash -LiteralPath (Join-Path $installed 'LICENSE') -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $portable 'LICENSE') -Algorithm SHA256).Hash) { throw 'Installed LICENSE differs' }
    $registrations = @(Rice-Registrations)
    if ($registrations.Count -ne 1 -or $registrations[0].DisplayVersion -ne $manifest.version) { throw 'Missing/ambiguous NSIS registration or version' }
    $registrationPath = $registrations[0].PSPath
    Probe-App $installedExe 'installed'
    $uninstaller = Join-Path $installed 'uninstall.exe'
    if (-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) { throw 'Missing NSIS uninstaller' }
    Run-Nsis $uninstaller "/S _?=$installed" 'silent-uninstall'
    if ((Test-Path -LiteralPath $installedExe) -or (Test-Path -LiteralPath (Join-Path $installed 'LICENSE')) -or (Test-Path -LiteralPath $registrationPath)) { throw 'NSIS uninstall left app/resources/registration behind' }
    $reportData.status = 'success'
} catch {
    $reportData.status = 'failure'
    $reportData.error = $_.Exception.Message
    throw
} finally {
    $reportData | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Report -Encoding utf8
    # The GUID scratch tree was created above under RUNNER_TEMP. Profile data
    # was proven absent before this disposable runner started the production app.
    if ($reportData.status -eq 'success') {
        Remove-Item -LiteralPath $scratch -Recurse -Force
        if (Test-Path -LiteralPath $profileData) { Remove-Item -LiteralPath $profileData -Recurse -Force }
    }
}
