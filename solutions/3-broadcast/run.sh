#!/usr/bin/env bash
set -xeuo pipefail

cargo build

# 3c, fault tolerance
maelstrom test \
  -w broadcast \
  --bin ../../target/debug/broadcast \
  --node-count 5 \
  --time-limit 20 \
  --rate 10 \
  --nemesis partition

# 3d, perf
maelstrom test \
  -w broadcast \
  --bin ../../target/debug/broadcast \
  --node-count 25 \
  --time-limit 20 \
  --rate 100 \
  --latency 100
