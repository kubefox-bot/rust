#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$')][string]$Name,
    [Parameter(Mandatory = $true)][string]$SwitchName,
    [Parameter(Mandatory = $true)][string]$Destination,
    [Parameter(Mandatory = $true)][string]$SshPublicKeyPath,
    [Parameter(Mandatory = $true)][string]$Origin,
    [ValidateRange(2, 64)][int]$MemoryGB = 4,
    [ValidateRange(2, 32)][int]$CpuCount = 2,
    [ValidateRange(16, 4096)][int]$DiskSizeGB = 64
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'installer-lib.ps1')
Install-AstraVm -Bundle $PSScriptRoot -Name $Name -SwitchName $SwitchName -Destination $Destination -SshPublicKeyPath $SshPublicKeyPath -Origin $Origin -MemoryGB $MemoryGB -CpuCount $CpuCount -DiskSizeGB $DiskSizeGB
