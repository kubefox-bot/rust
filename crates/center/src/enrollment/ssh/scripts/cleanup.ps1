$path = Stage-Path $request
if ([IO.File]::Exists($path)) {
    $verified = Verify-Package $request
    [IO.File]::Delete($verified)
}
@{ok=$true} | ConvertTo-Json -Compress
