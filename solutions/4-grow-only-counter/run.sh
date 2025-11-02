#!/usr/bin/env bash
set -xeuo pipefail

cargo build

maelstrom test \
  -w g-counter \
  --bin ../../target/debug/grow-only-counter \
  --node-count 3 \
  --time-limit 20 \
  --rate 100 --nemesis partition