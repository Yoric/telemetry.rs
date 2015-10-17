#!/bin/bash
set -ex

# Run all the usual Rust tests.
cargo test --no-run
cargo test
r=${PIPESTATUS[0]}
if [ $r -ne 0 ]; then exit $r; fi

# Build the C API and try compiling a C program against it.
cargo test --manifest-path capi/Cargo.toml
r=${PIPESTATUS[0]}
if [ $r -ne 0 ]; then exit $r; fi
make -C examples/capi
