#!/usr/bin/env bash
# Run the repository checks and print a compact, recording-friendly summary.
set -uo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
LOG_DIR=$(mktemp -d "${TMPDIR:-/tmp}/kv-check.XXXXXX")
trap 'rm -rf "$LOG_DIR"' EXIT

PASSED=0
FAILED=0

run_step() {
  local name="$1"
  shift
  local log_file="$LOG_DIR/${PASSED}_${FAILED}_${name//[^A-Za-z0-9_.-]/_}.log"

  printf '\n[%s]\n' "$name"
  printf '  %s\n' "$*"

  if "$@" >"$log_file" 2>&1; then
    PASSED=$((PASSED + 1))
    printf '  PASS\n'
  else
    local status=$?
    FAILED=$((FAILED + 1))
    printf '  FAIL (exit %s)\n' "$status"
  fi

  if [[ -s "$log_file" ]]; then
    tail -n 8 "$log_file" | sed 's/^/  /'
  else
    printf '  (no output)\n'
  fi
}

run_shell_step() {
  local name="$1"
  local command="$2"
  local log_file="$LOG_DIR/${PASSED}_${FAILED}_${name//[^A-Za-z0-9_.-]/_}.log"

  printf '\n[%s]\n' "$name"
  printf '  %s\n' "$command"

  if (bash -lc "cd \"$ROOT_DIR\" && $command" >"$log_file" 2>&1); then
    PASSED=$((PASSED + 1))
    printf '  PASS\n'
  else
    local status=$?
    FAILED=$((FAILED + 1))
    printf '  FAIL (exit %s)\n' "$status"
  fi

  if [[ -s "$log_file" ]]; then
    tail -n 8 "$log_file" | sed 's/^/  /'
  else
    printf '  (no output)\n'
  fi
}

printf '%s\n' 'KV Database - verification summary'
printf '%s\n' '=================================='
printf 'root: %s\n' "$ROOT_DIR"
printf 'logs: %s (removed on exit)\n' "$LOG_DIR"
printf '%s\n' 'Run this script before starting a demo server on port 3307.'

run_step 'Rust format' cargo fmt --check --all
run_step 'Rust clippy' cargo clippy --workspace --all-targets -- -D warnings
run_step 'Rust tests' cargo test --workspace --all-targets
run_step 'Rust docs' cargo doc --workspace --no-deps
run_shell_step 'Frontend build' 'cd demo-client && npm run build'
run_step 'Protocol and persistence tests' python test_protocol.py
run_step 'Diff whitespace' git diff --check

printf '\n%s\n' '=================================='
printf 'passed: %s\n' "$PASSED"
printf 'failed: %s\n' "$FAILED"

if [[ "$FAILED" -eq 0 ]]; then
  printf '%s\n' 'RESULT: ALL CHECKS PASSED'
  exit 0
fi

printf '%s\n' 'RESULT: CHECKS FAILED (see the failed step output above)'
exit 1

