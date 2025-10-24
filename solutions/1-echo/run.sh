#!/usr/bin/env bash
set -xeuo pipefail

cargo build

maelstrom test \
  -w echo \
  --bin ../../target/debug/echo \
  --node-count 1 \
  --time-limit 10
