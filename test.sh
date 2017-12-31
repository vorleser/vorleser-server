#!/bin/bash

watchexec --exts rs,toml,sql --restart "pwd && cargo build && RUST_BACKTRACE=1 cargo test"
