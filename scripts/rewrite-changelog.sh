#!/usr/bin/env bash
# Reads a release-plz / git-cliff changelog section on stdin and prints a
# user-focused rewrite on stdout.
#
# Designed to be wired into release-plz.toml as a [changelog] postprocessor:
#
#     [changelog]
#     postprocessors = [
#       { pattern = '(?s).*', replace_command = 'scripts/rewrite-changelog.sh' },
#     ]
#
# Behavior:
#   * Reads entire stdin.
#   * If the input is empty or doesn't look like a release section
#     (no `## [` heading), passes it through unchanged.
#   * Otherwise, invokes `claude -p` to rewrite the section, keeping only
#     entries that matter to library consumers (features and fixes that
#     affect behavior or public API). Internal refactors, CI, chores, dev
#     docs, and test changes are dropped.
#   * Caches successful rewrites by SHA-256 of stdin so repeated invocations
#     with the same input (release-plz calls the postprocessor several
#     times per release) only pay for one Claude call.
#   * On any failure (claude missing, non-zero exit, suspicious output),
#     prints the original input unchanged so release-plz never breaks.

set -uo pipefail

INPUT="$(cat)"

# Only rewrite real release sections — must contain a versioned heading
# (## [X.Y...]) AND at least one bullet. Skip the file header and skip
# empty queries.
if [ -z "$INPUT" ] \
  || ! printf '%s' "$INPUT" | grep -Eq '^## \[[0-9]' \
  || ! printf '%s' "$INPUT" | grep -Eq '^- '; then
  printf '%s' "$INPUT"
  exit 0
fi

CACHE_DIR="${CHANGELOG_REWRITE_CACHE_DIR:-${TMPDIR:-/tmp}/toasty-changelog-rewrite}"
mkdir -p "$CACHE_DIR"
HASH="$(printf '%s' "$INPUT" | shasum -a 256 | awk '{print $1}')"
CACHE_FILE="$CACHE_DIR/$HASH.md"

if [ -f "$CACHE_FILE" ]; then
  cat "$CACHE_FILE"
  exit 0
fi

if ! command -v claude >/dev/null 2>&1; then
  printf '%s' "$INPUT"
  exit 0
fi

read -r -d '' PROMPT <<'PROMPT_EOF' || true
You are rewriting one release section of a Rust crate changelog so that only changes that matter to LIBRARY CONSUMERS appear. Output ONLY the rewritten markdown — no preamble, no commentary, no code fences, no XML tags, no system reminders.

Rules:

KEEP:
  - New public API, new features, new query/macro capabilities.
  - Bug fixes that affect runtime behavior, panics, query results, or codegen.
  - Breaking changes (mark with [**breaking**] if not already marked).
  - New driver or database support.
  - User-visible performance improvements.

DROP:
  - Internal refactors and code reorganizations with no API change.
  - CI / tooling / lint / clippy / formatting changes.
  - Test-only changes.
  - Dev-doc updates (design docs, contributing guides, internal architecture notes).
  - README typo or link fixes that don't change documented behavior.
  - Dependency bumps unless they raise MSRV or change a public re-export.
  - Typo fixes in code comments.
  - Renames that are not visible in the public API.

FORMAT:
  - Preserve the version heading line (## [...]...) exactly as given.
  - Use ### Added, ### Fixed, and ### Changed subsections only. Drop ### Other entirely (move kept items into the right section, or drop them).
  - Preserve PR links exactly as given (e.g. ([#123](...))).
  - Rewrite each kept bullet so it reads as a user-facing benefit. Drop implementation jargon. Use sentence case. No trailing period.
  - Group related entries; merge near-duplicates into a single line.
  - Omit empty subsections.
  - If no entries are user-relevant at all, output the version heading followed by a blank line and one bullet: "- Internal improvements only."
  - End with exactly one trailing newline.

Input changelog section:

PROMPT_EOF

OUTPUT="$(printf '%s%s\n' "$PROMPT" "$INPUT" | claude -p \
  --model claude-haiku-4-5 \
  --setting-sources '' \
  --disable-slash-commands \
  --permission-mode plan \
  2>/dev/null)"

# Sanity-check the output: must be non-empty, must not be a login error,
# must contain the original version heading line.
HEADING="$(printf '%s' "$INPUT" | grep -m1 '^## \[' || true)"
if [ -z "$OUTPUT" ] \
  || printf '%s' "$OUTPUT" | grep -qi 'not logged in\|please run /login\|api error' \
  || ! printf '%s' "$OUTPUT" | grep -qF "$HEADING"; then
  printf '%s' "$INPUT"
  exit 0
fi

# Strip any leaked <system-reminder>...</system-reminder> blocks and trim any
# leading or trailing blank lines from Claude output. release-plz/git-cliff
# expect each rendered section to be wrapped with one leading and one trailing
# blank line so that consecutive sections in the assembled CHANGELOG.md are
# separated, so we re-add those wrappers ourselves below.
OUTPUT="$(printf '%s\n' "$OUTPUT" | awk '
  /<system-reminder>/ { skip=1; next }
  /<\/system-reminder>/ { skip=0; next }
  !skip { lines[++n] = $0 }
  END {
    s = 1; while (s <= n && lines[s] == "") s++
    e = n; while (e >= s && lines[e] == "") e--
    for (i = s; i <= e; i++) print lines[i]
  }
')"

FINAL=$'\n'"$OUTPUT"$'\n'
printf '%s' "$FINAL" >"$CACHE_FILE"
printf '%s' "$FINAL"
