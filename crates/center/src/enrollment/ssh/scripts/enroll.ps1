$package = Verify-Package $request
$data = Join-Path $env:ProgramData 'hMailCleanerAgent'
$arguments = @('agent','enroll','--data-dir',$data,'--agent-id',[string]$request.agent_id)
& $package @arguments
if ($LASTEXITCODE -ne 0) { throw 'Enrollment failed' }
