#!/bin/bash

watchexec --exts rs,toml,sql --restart "pwd && ./reset.sh && cargo build && RUST_BACKTRACE=1 cargo test"
