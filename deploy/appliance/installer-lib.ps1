Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-AstraBundle {
    param([string]$Bundle)
    $manifestPath = Join-Path $Bundle 'appliance-manifest.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.format_version -ne 1 -or $manifest.product -ne 'astra-hyperv' -or $manifest.platform -ne 'linux/amd64' -or $manifest.schema_version -ne 2 -or $manifest.source_sha -notmatch '^[0-9a-f]{40}$' -or $manifest.build -cne "git-$($manifest.source_sha)") {
        throw 'Unsupported appliance identity.'
    }
    $expected = @('astra-center.vhdx', 'Install-Astra.ps1', 'installer-lib.ps1', 'kvpctl.exe', 'base.ign', 'README.txt', 'assembly-report.json')
    $names = @($manifest.files | ForEach-Object { $_.name })
    if (@(Compare-Object ($expected | Sort-Object) ($names | Sort-Object)).Count -ne 0 -or @($names | Select-Object -Unique).Count -ne $expected.Count) { throw 'Unexpected appliance files.' }
    foreach ($file in $manifest.files) {
        if ($file.sha256 -notmatch '^[0-9a-f]{64}$' -or $file.size -le 0) { throw 'Invalid appliance checksum.' }
        $item = Get-Item -LiteralPath (Join-Path $Bundle $file.name)
        if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $item.Length -ne $file.size -or (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -cne $file.sha256) { throw "Appliance checksum mismatch: $($file.name)" }
    }
    return $manifest
}
function New-AstraIgnition {
    param([string]$Bundle, [string]$SshPublicKeyPath, [string]$Origin)
    $keyFile = Get-Item -LiteralPath $SshPublicKeyPath
    if ($keyFile.Length -gt 16384 -or $keyFile.PSIsContainer -or ($keyFile.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Invalid SSH public key file.' }
    $key = [IO.File]::ReadAllText($keyFile.FullName).Trim()
    if ($key -notmatch '^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(256|384|521)) [A-Za-z0-9+/]+={0,3}( [^\r\n]*)?$') { throw 'Supply one OpenSSH public key, without authorized_keys options.' }
    $keyParts = $key.Split(' ')
    $blob = [Convert]::FromBase64String($keyParts[1])
    if ($blob.Length -lt 4) { throw 'Truncated SSH public key.' }
    $typeLength = [Net.IPAddress]::NetworkToHostOrder([BitConverter]::ToInt32($blob, 0))
    if ($typeLength -lt 1 -or $typeLength -gt 64 -or $blob.Length -lt (4 + $typeLength + 32) -or [Text.Encoding]::ASCII.GetString($blob, 4, $typeLength) -cne $keyParts[0]) { throw 'Invalid SSH public key encoding.' }
    $address = $null
    if (-not [Uri]::TryCreate($Origin, [UriKind]::Absolute, [ref]$address) -or $address.Scheme -cne 'https' -or $address.UserInfo -or $address.AbsolutePath -cne '/' -or $address.Query -or $address.Fragment) { throw 'Origin must be an HTTPS origin without credentials or path.' }
    $config = Get-Content -LiteralPath (Join-Path $Bundle 'base.ign') -Raw | ConvertFrom-Json
    $core = @($config.passwd.users | Where-Object { $_.name -ceq 'core' })
    if ($core.Count -ne 1) { throw 'Invalid administrative user template.' }
    $core[0].sshAuthorizedKeys = @($key)
    $originFile = @($config.storage.files | Where-Object { $_.path -ceq '/var/lib/astra-appliance-install/origin' })
    if ($originFile.Count -ne 1) { throw 'Invalid origin template.' }
    $originFile[0].contents.source = 'data:;base64,' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($address.GetLeftPart([UriPartial]::Authority) + "`n"))
    return ($config | ConvertTo-Json -Depth 100)
}
function Assert-AstraTarget {
    param([string]$Name, [string]$SwitchName, [string]$Destination)
    if (@(Get-VM | Where-Object { $_.Name -ieq $Name }).Count -ne 0) { throw 'A VM with this name already exists; nothing changed.' }
    $switch = @(Get-VMSwitch | Where-Object { $_.Name -ceq $SwitchName })
    if ($switch.Count -ne 1) { throw 'Choose one existing virtual switch; no switch will be created.' }
    $directory = [IO.Path]::GetFullPath($Destination)
    if (-not [IO.Path]::IsPathRooted($Destination) -or (Test-Path -LiteralPath $directory)) { throw 'Destination must be a new absolute directory.' }
    $parent = Get-Item -LiteralPath (Split-Path -Parent $directory)
    if (-not $parent.PSIsContainer -or ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Unsafe destination parent.' }
    return $directory
}
function Set-AstraIgnition {
    param([string]$Bundle, [string]$Name, [string]$Path)
    & (Join-Path $Bundle 'kvpctl.exe') $Name add-ign $Path
    if ($LASTEXITCODE -ne 0) { throw 'Ignition KVP injection failed; VM was not started.' }
}
function New-AstraVm {
    param([string]$Bundle, [string]$Name, [string]$SwitchName, [string]$Directory, [string]$Ignition, [int]$MemoryGB, [int]$CpuCount, [int]$DiskSizeGB)
    New-Item -ItemType Directory -Path $Directory -ErrorAction Stop | Out-Null
    try {
        $disk = Join-Path $directory 'astra-center.vhdx'
        Copy-Item -LiteralPath (Join-Path $Bundle 'astra-center.vhdx') -Destination $disk
        Resize-VHD -Path $disk -SizeBytes ($DiskSizeGB * 1GB)
        $ignitionPath = Join-Path $directory 'config.ign'
        [IO.File]::WriteAllText($ignitionPath, $ignition, [Text.UTF8Encoding]::new($false))
        $vm = New-VM -Name $Name -Generation 2 -MemoryStartupBytes ($MemoryGB * 1GB) -VHDPath $disk -SwitchName $SwitchName -Path $directory
        Set-VMProcessor -VM $vm -Count $CpuCount
        Set-VMFirmware -VM $vm -EnableSecureBoot On -SecureBootTemplate MicrosoftUEFICertificateAuthority
        Set-AstraIgnition -Bundle $Bundle -Name $Name -Path $ignitionPath
        Start-VM -VM $vm
    } catch {
        Write-Warning "Installation stopped. Only resources created under $directory and VM $Name may be partial; they were retained. Inspect them before a retry."
        throw
    }
}
function Install-AstraVm {
    param([string]$Bundle, [string]$Name, [string]$SwitchName, [string]$Destination, [string]$SshPublicKeyPath, [string]$Origin, [int]$MemoryGB, [int]$CpuCount, [int]$DiskSizeGB)
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not ([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run Windows PowerShell as Administrator.' }
    Import-Module Hyper-V -ErrorAction Stop
    $manifest = Assert-AstraBundle -Bundle $Bundle
    $ignition = New-AstraIgnition -Bundle $Bundle -SshPublicKeyPath $SshPublicKeyPath -Origin $Origin
    $directory = Assert-AstraTarget -Name $Name -SwitchName $SwitchName -Destination $Destination
    # All validation precedes the first mutation. Partial resources are retained for diagnosis.
    New-AstraVm -Bundle $Bundle -Name $Name -SwitchName $SwitchName -Directory $directory -Ignition $ignition -MemoryGB $MemoryGB -CpuCount $CpuCount -DiskSizeGB $DiskSizeGB
    Write-Host "Created $Name ($($manifest.build)). Use DHCP to find its address, SSH as core, then run sudo -iu hmail-center /var/home/hmail-center/.local/bin/astra-initialize."
}
