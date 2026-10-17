$package = Verify-Package $request
$data = Join-Path $env:ProgramData 'hMailCleanerAgent'
$config = Join-Path $data 'agent.json'
Assert-FlatDirectory $data
$identity = [IO.File]::ReadAllText($config) | ConvertFrom-Json
if ($identity.agent_id -ne $request.agent_id -or $identity.center_id -ne $request.center_id) { throw 'Wrong installation identity' }
& $package @('agent','prepare-recovery','--config',$config)
if ($LASTEXITCODE -ne 0) { throw 'Agent is not ready for trust recovery' }
