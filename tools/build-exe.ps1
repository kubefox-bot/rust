param([switch]$Test)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path 'target/build-driver' | Out-Null
if ($Test) {
    rustc --edition=2021 --test tools/build-exe/main.rs -o target/build-driver/build-tests.exe
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & './target/build-driver/build-tests.exe'
} else {
    rustc --edition=2021 tools/build-exe/main.rs -o target/build-driver/build-exe.exe
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & './target/build-driver/build-exe.exe'
}
exit $LASTEXITCODE
