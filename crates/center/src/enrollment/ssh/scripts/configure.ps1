$bootstrap = $request.bootstrap
$data = Join-Path $env:ProgramData 'hMailCleanerAgent'
Assert-FlatDirectory $data
if (-not [IO.File]::Exists((Join-Path $data 'identity-key.pem'))) { throw 'Missing local identity key' }
if ($request.listen_port -lt 1 -or $request.listen_port -gt 65535) { throw 'Invalid agent port' }
$encoding = [Text.UTF8Encoding]::new($false)
$values = @{
    'identity.pem'=[string]$request.certificate_pem
    'trust.pem'=[string]$request.ca_pem
    'agent.json'=(@{listen=('0.0.0.0:' + $request.listen_port);agent_id=$bootstrap.agent_id;center_id=$bootstrap.center_id} | ConvertTo-Json -Compress)
}
foreach ($name in $values.Keys) {
    $path = Join-Path $data $name
    $bytes = $encoding.GetBytes($values[$name])
    $file = [IO.FileStream]::new($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try { $file.Write($bytes,0,$bytes.Length);$file.Flush($true) } finally { $file.Dispose() }
}
@{ok=$true} | ConvertTo-Json -Compress
