$data = Join-Path $env:ProgramData 'hMailCleanerAgent'
Assert-FlatDirectory $data
$identity = [IO.File]::ReadAllText((Join-Path $data 'agent.json')) | ConvertFrom-Json
if ($identity.agent_id -ne $request.bootstrap.agent_id -or $identity.center_id -ne $request.bootstrap.center_id) { throw 'Wrong installation identity' }
$encoding = [Text.UTF8Encoding]::new($false)
foreach ($pair in @(@('identity.pem',[string]$request.certificate_pem),@('trust.pem',[string]$request.ca_pem))) {
    $destination = Join-Path $data $pair[0]
    $temporary = $destination + '.new'
    $bytes = $encoding.GetBytes($pair[1])
    $file = [IO.FileStream]::new($temporary,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try { $file.Write($bytes,0,$bytes.Length);$file.Flush($true) } finally { $file.Dispose() }
    [IO.File]::Replace($temporary,$destination,$null)
}
@{ok=$true} | ConvertTo-Json -Compress
