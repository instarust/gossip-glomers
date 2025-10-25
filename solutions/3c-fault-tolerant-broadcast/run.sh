#!/usr/bin/env bash
set -xeuo pipefail

cargo build

maelstrom test \
  -w broadcast \
  --bin ../../target/debug/single-node-broadcast \
  --node-count 1 \
  --time-limit 20 \
  --rate 10
