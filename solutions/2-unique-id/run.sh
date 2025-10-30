#!/usr/bin/env bash
set -xeuo pipefail

cargo build


maelstrom test \
  -w unique-ids \
  --bin ../../target/debug/unique-id \
  --time-limit 30 \
  --rate 1000 \
  --node-count 3 \
  --availability total \
  --nemesis partition
