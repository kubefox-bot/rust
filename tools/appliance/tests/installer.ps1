#requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
. (Join-Path $root 'deploy/appliance/installer-lib.ps1')
function Assert-Throws {
    param([scriptblock]$Operation, [string]$Message)
    $failed = $false
    try { & $Operation } catch { $failed = $true }
    if (-not $failed) { throw $Message }
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('astra-installer-' + [Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    foreach ($file in @('Install-Astra.ps1', 'installer-lib.ps1')) {
        $tokens = $null
        $errors = $null
        [Management.Automation.Language.Parser]::ParseFile((Join-Path $root "deploy/appliance/$file"), [ref]$tokens, [ref]$errors) | Out-Null
        if ($errors.Count -ne 0) { throw "PowerShell parser rejected $file" }
    }
    function Get-VM { @([pscustomobject]@{ Name = 'Existing' }) }
    function Get-VMSwitch { @([pscustomobject]@{ Name = 'External' }) }
    Assert-Throws { Assert-AstraTarget -Name Existing -SwitchName External -Destination (Join-Path $temporary 'new') } 'Existing VM was accepted.'
    Assert-Throws { Assert-AstraTarget -Name New -SwitchName Missing -Destination (Join-Path $temporary 'new') } 'Missing switch was accepted.'
    Assert-Throws { Assert-AstraTarget -Name New -SwitchName External -Destination $temporary } 'Existing directory was accepted.'
    $destination = Assert-AstraTarget -Name New -SwitchName External -Destination (Join-Path $temporary 'new')
    if (Test-Path -LiteralPath $destination) { throw 'Preflight mutated the destination.' }
    $base = @{ passwd = @{ users = @(@{ name = 'core'; sshAuthorizedKeys = @() }) }; storage = @{ files = @(@{ path = '/var/lib/astra-appliance-install/origin'; contents = @{ source = '' } }) } }
    [IO.File]::WriteAllText((Join-Path $temporary 'base.ign'), ($base | ConvertTo-Json -Depth 20))
    $key = Join-Path $temporary 'admin.pub'
    [IO.File]::WriteAllText($key, '-----BEGIN OPENSSH PRIVATE KEY-----')
    Assert-Throws { New-AstraIgnition -Bundle $temporary -SshPublicKeyPath $key -Origin https://example.test:8443 } 'Private key was accepted.'
    [IO.File]::WriteAllText($key, 'ssh-ed25519 AAAA')
    Assert-Throws { New-AstraIgnition -Bundle $temporary -SshPublicKeyPath $key -Origin https://example.test:8443 } 'Truncated key was accepted.'
    $keygen = Join-Path $temporary 'operator'
    & ssh-keygen -q -t ed25519 -N '""' -f $keygen
    if ($LASTEXITCODE -ne 0) { throw 'Fixture key generation failed.' }
    $publicKey = $keygen + '.pub'
    $config = New-AstraIgnition -Bundle $temporary -SshPublicKeyPath $publicKey -Origin https://example.test:8443 | ConvertFrom-Json
    if ($config.passwd.users[0].sshAuthorizedKeys.Count -ne 1) { throw 'Public key was not inserted.' }
    Assert-Throws { New-AstraIgnition -Bundle $temporary -SshPublicKeyPath $publicKey -Origin 'https://user:password@example.test' } 'Credential-bearing origin was accepted.'
    Assert-Throws { New-AstraIgnition -Bundle $temporary -SshPublicKeyPath $publicKey -Origin 'https://example.test/path' } 'Origin with path was accepted.'
    $names = @('astra-center.vhdx', 'Install-Astra.ps1', 'installer-lib.ps1', 'kvpctl.exe', 'base.ign', 'README.txt', 'assembly-report.json')
    foreach ($name in $names) { if (-not (Test-Path -LiteralPath (Join-Path $temporary $name))) { [IO.File]::WriteAllText((Join-Path $temporary $name), 'fixture') } }
    $files = @($names | ForEach-Object { $item = Get-Item -LiteralPath (Join-Path $temporary $_); @{ name = $_; size = $item.Length; sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } })
    $sourceSha = 'a' * 40
    $manifest = @{ format_version = 1; product = 'astra-hyperv'; platform = 'linux/amd64'; schema_version = 2; source_sha = $sourceSha; build = "git-$sourceSha"; files = $files }
    [IO.File]::WriteAllText((Join-Path $temporary 'appliance-manifest.json'), ($manifest | ConvertTo-Json -Depth 20))
    Assert-AstraBundle -Bundle $temporary | Out-Null
    [IO.File]::WriteAllText((Join-Path $temporary 'kvpctl.exe'), 'changed')
    Assert-Throws { Assert-AstraBundle -Bundle $temporary } 'Modified KVP executable was accepted.'
    $script:effects = @()
    function Resize-VHD { param($Path, $SizeBytes) $script:effects += 'resize' }
    function New-VM { param($Name, $Generation, $MemoryStartupBytes, $VHDPath, $SwitchName, $Path) $script:effects += 'create'; return [pscustomobject]@{ Name = $Name } }
    function Set-VMProcessor { param($VM, $Count) $script:effects += 'cpu' }
    function Set-VMFirmware { param($VM, $EnableSecureBoot, $SecureBootTemplate) $script:effects += 'firmware' }
    function Set-AstraIgnition { param($Bundle, $Name, $Path) $script:effects += 'kvp'; throw 'fixture KVP failure' }
    function Start-VM { param($VM) $script:effects += 'start' }
    $partial = Join-Path $temporary 'partial'
    try {
        New-AstraVm -Bundle $temporary -Name Fixture -SwitchName External -Directory $partial -Ignition '{}' -MemoryGB 4 -CpuCount 2 -DiskSizeGB 64
        throw 'Expected KVP failure.'
    } catch { if ($_.Exception.Message -notmatch 'fixture KVP failure') { throw } }
    if (($script:effects -join ',') -cne 'resize,create,cpu,firmware,kvp') { throw 'VM start was attempted before successful KVP injection.' }
    if (-not (Test-Path -LiteralPath (Join-Path $partial 'astra-center.vhdx'))) { throw 'Partial resources were deleted.' }
    Write-Host "PASS: installer validation fixtures in PowerShell $($PSVersionTable.PSVersion). No VM was created."
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
