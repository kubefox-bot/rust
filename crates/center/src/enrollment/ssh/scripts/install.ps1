$bootstrap = $request.bootstrap
$package = Verify-Package $bootstrap
$data = Join-Path $env:ProgramData 'hMailCleanerAgent'
$config = Join-Path $data 'agent.json'
Assert-FlatDirectory $data
$identity = [IO.File]::ReadAllText($config) | ConvertFrom-Json
if ($identity.agent_id -ne $bootstrap.agent_id -or $identity.center_id -ne $bootstrap.center_id) { throw 'Wrong installation identity' }
$exe = Join-Path (Join-Path $env:ProgramFiles 'hMailCleanerAgent') 'hmail-cleaner-agent.exe'
$arguments = @('agent','install-service','--operation-id',[string]$bootstrap.operation_id,'--package',$package,'--executable',$exe,'--config',$config,'--sha256',[string]$bootstrap.sha256)
if ($null -ne $request.legacy) {
    $arguments += @('--legacy-executable',[string]$request.legacy.executable,'--legacy-env',[string]$request.legacy.env_file)
}
& $package @arguments
if ($LASTEXITCODE -ne 0) { throw 'Service installation failed' }
