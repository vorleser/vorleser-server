#!/bin/bash

watchexec --exts rs,toml,sql --restart "pwd && ./reset.sh && RUST_BACKTRACE=1 cargo run"