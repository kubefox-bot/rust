use super::InstallError;
use std::{fs, path::Path};

pub fn directory(path: &Path) -> Result<(), InstallError> {
    if !path.is_absolute() {
        return Err(InstallError::Invalid);
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        builder.mode(0o700);
        builder.create(path)?;
        let info = fs::symlink_metadata(path)?;
        if !info.is_dir() || info.file_type().is_symlink() {
            return Err(InstallError::Invalid);
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    {
        // Provision only the dedicated agent directory, never a shared system parent.
        if !path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case(super::SERVICE_NAME)
        }) || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        }) {
            return Err(InstallError::Invalid);
        }
        if !matches!(path.components().next(), Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
        {
            return Err(InstallError::Invalid);
        }
        builder.create(path)?;
        let info = fs::symlink_metadata(path)?;
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if !info.is_dir() || info.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(InstallError::Invalid);
        }
        // Replace the DACL atomically; granting rights with icacls would retain
        // pre-existing explicit ACEs. No caller text is interpolated into code.
        const ACL_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$acl = [System.Security.AccessControl.DirectorySecurity]::new()
$acl.SetAccessRuleProtection($true, $false)
foreach ($value in @('S-1-5-18', 'S-1-5-32-544')) {
    $sid = [System.Security.Principal.SecurityIdentifier]::new($value)
    $rule = [System.Security.AccessControl.FileSystemAccessRule]::new($sid, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
    $acl.AddAccessRule($rule)
}
[System.IO.Directory]::SetAccessControl($env:HMAIL_AGENT_DIRECTORY, $acl)
foreach ($entry in [System.IO.Directory]::EnumerateFileSystemEntries($env:HMAIL_AGENT_DIRECTORY)) {
    $attributes = [System.IO.File]::GetAttributes($entry)
    if (($attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or ($attributes -band [System.IO.FileAttributes]::Directory) -ne 0) {
        throw 'Agent state and executable directories must be flat and contain no reparse points'
    }
    $fileAcl = [System.Security.AccessControl.FileSecurity]::new()
    $fileAcl.SetAccessRuleProtection($true, $false)
    foreach ($value in @('S-1-5-18', 'S-1-5-32-544')) {
        $sid = [System.Security.Principal.SecurityIdentifier]::new($value)
        $fileAcl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($sid, 'FullControl', 'Allow'))
    }
    [System.IO.File]::SetAccessControl($entry, $fileAcl)
}
"#;
        let success = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", ACL_SCRIPT])
            .env("HMAIL_AGENT_DIRECTORY", path)
            .output()?
            .status
            .success();
        if !success {
            return Err(InstallError::Native);
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err(InstallError::Unsupported);
    }
    Ok(())
}
