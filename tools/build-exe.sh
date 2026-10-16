#!/bin/sh
set -eu
mkdir -p target/build-driver
case "${1-}" in
  '')
    rustc --edition=2021 tools/build-exe/main.rs -o target/build-driver/build-exe
    target/build-driver/build-exe
    ;;
  --test)
    rustc --edition=2021 --test tools/build-exe/main.rs -o target/build-driver/build-tests
    target/build-driver/build-tests
    ;;
  *) exit 2 ;;
esac
