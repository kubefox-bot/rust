$stage = Join-Path $env:ProgramData 'hMailCleanerAgentStage'
$acl = [Security.AccessControl.DirectorySecurity]::new()
$acl.SetAccessRuleProtection($true, $false)
foreach ($value in @('S-1-5-18', 'S-1-5-32-544')) {
    $sid = [Security.Principal.SecurityIdentifier]::new($value)
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
}
if ([IO.Directory]::Exists($stage)) {
    Assert-FlatDirectory $stage
    $existing = [IO.Directory]::GetAccessControl($stage)
    $owner = $existing.GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -notin @('S-1-5-18', 'S-1-5-32-544')) { throw 'Untrusted staging owner' }
    if (-not $existing.AreAccessRulesProtected) { throw 'Untrusted staging inheritance' }
    foreach ($rule in $existing.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.IdentityReference.Value -notin @('S-1-5-18', 'S-1-5-32-544')) { throw 'Untrusted staging ACL' }
    }
} else {
    $directory = [IO.DirectoryInfo]::new($stage)
    $directory.Create($acl)
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
    [IO.Directory]::SetAccessControl($stage, $acl)
}
$path = Stage-Path $request
if ([IO.File]::Exists($path)) { throw 'Existing staged operation requires inspection' }
@{path=$path.Replace('\','/')} | ConvertTo-Json -Compress
