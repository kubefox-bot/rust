$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$inputJson = [Console]::In.ReadToEnd()
if ($inputJson.Length -gt 65536) { throw 'Bootstrap input limit' }
$request = $inputJson | ConvertFrom-Json
function Assert-Platform {
    $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Administrator required' }
    if (-not [Environment]::Is64BitProcess -or $PSVersionTable.PSVersion -lt [version]'5.1') { throw '64-bit PowerShell 5.1 required' }
    if ([version](Get-CimInstance Win32_OperatingSystem).Version -lt [version]'10.0.14393') { throw 'Unsupported Windows version' }
    if ((Get-CimInstance Win32_Processor | Select-Object -First 1).Architecture -ne 9) { throw 'x64 Windows required' }
}
function Assert-FlatDirectory([string]$path) {
    if ([IO.Directory]::Exists($path)) {
        if (([IO.File]::GetAttributes($path) -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Reparse directory' }
        foreach ($item in [IO.Directory]::EnumerateFileSystemEntries($path)) {
            if (([IO.File]::GetAttributes($item) -band ([IO.FileAttributes]::ReparsePoint -bor [IO.FileAttributes]::Directory)) -ne 0) { throw 'Non-flat directory' }
        }
    }
}
function Stage-Path($bootstrap) {
    if ($bootstrap.operation_id -notmatch '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$') { throw 'Invalid operation UUID' }
    Join-Path (Join-Path $env:ProgramData 'hMailCleanerAgentStage') ($bootstrap.operation_id + '.exe')
}
function Verify-Package($bootstrap) {
    if ($bootstrap.sha256 -notmatch '^[0-9a-f]{64}$') { throw 'Invalid checksum' }
    $package = Stage-Path $bootstrap
    if (([IO.File]::GetAttributes($package) -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Reparse package' }
    if ((Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant() -ne $bootstrap.sha256) { throw 'Checksum mismatch' }
    $package
}
Assert-Platform
