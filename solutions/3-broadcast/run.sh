#!/usr/bin/env bash
set -xeuo pipefail

cargo build

maelstrom test \
  -w broadcast \
  --bin ../../target/debug/single-node-broadcast \
  --node-count 5 \
  --time-limit 20 \
  --rate 10 --nemesis partition