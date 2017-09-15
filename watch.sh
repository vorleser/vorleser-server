#!/bin/bash

watchexec --exts rs,toml,sql --restart "cargo build && RUST_BACKTRACE=1 cargo run -- ${1:-serve}"
