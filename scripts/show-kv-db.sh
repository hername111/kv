#!/usr/bin/env bash
# Read-only inspection helper for the custom KV Database file format.
set -euo pipefail

DB_PATH="${1:-${KV_DATA_DIR:-kv_data}/kv.db}"
PAGE_SIZE=4096

if [[ ! -f "$DB_PATH" ]]; then
  printf 'Database file not found: %s\n' "$DB_PATH" >&2
  printf 'Start kv-server first, or pass the path explicitly:\n' >&2
  printf '  %s path/to/kv.db\n' "$0" >&2
  exit 1
fi

if ! command -v od >/dev/null 2>&1; then
  printf 'This script requires od (available in GNU/Linux, macOS and Git Bash).\n' >&2
  exit 1
fi

FILE_BYTES=$(wc -c < "$DB_PATH" | tr -d ' ')
PAGE_COUNT=$((FILE_BYTES / PAGE_SIZE))
REMAINDER=$((FILE_BYTES % PAGE_SIZE))

read_u64_le() {
  od -An -tu8 -N8 -j"$1" "$DB_PATH" | tr -d ' '
}

read_u32_le() {
  od -An -tu4 -N4 -j"$1" "$DB_PATH" | tr -d ' '
}

MAGIC=$(dd if="$DB_PATH" bs=1 skip=24 count=8 2>/dev/null | LC_ALL=C tr -cd '[:print:]')
NEXT_PAGE=$(read_u64_le 0)
FREE_HEAD=$(read_u64_le 8)
CATALOG_ROOT=$(read_u64_le 16)
FORMAT_VERSION=$(read_u32_le 32)

printf '%s\n' 'KV Database file inspection (read-only)'
printf '%s\n' '--------------------------------------'
printf 'path:             %s\n' "$DB_PATH"
printf 'file size:        %s bytes\n' "$FILE_BYTES"
printf 'page size:        %s bytes\n' "$PAGE_SIZE"
printf 'page count:       %s\n' "$PAGE_COUNT"
printf 'superblock magic: %s\n' "${MAGIC:-<empty>}"
printf 'format version:   %s\n' "${FORMAT_VERSION:-<unreadable>}"
printf 'next page id:     %s\n' "${NEXT_PAGE:-<unreadable>}"
printf 'free-list head:   %s\n' "${FREE_HEAD:-<unreadable>}"
printf 'catalog root:     %s\n' "${CATALOG_ROOT:-<unreadable>}"

if [[ "$REMAINDER" -ne 0 ]]; then
  printf 'WARNING: file size is not page-aligned (remainder %s bytes)\n' "$REMAINDER"
fi

if [[ "${MAGIC:-}" != "KVDBPAGE" ]]; then
  printf 'WARNING: expected magic KVDBPAGE\n'
fi

if [[ "${NEXT_PAGE:-0}" -gt "$PAGE_COUNT" ]]; then
  printf 'WARNING: next page id exceeds physical page count\n'
fi

