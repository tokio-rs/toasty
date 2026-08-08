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
#   * Strips the inline URL from each `[#NNN](url)` PR reference and
#     appends a sorted `[#NNN]: <url>` definitions block at the end of
#     the section, so URLs render once at the bottom (Tokio-style)
#     instead of cluttering every bullet.
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
You are rewriting one release section of a Rust crate changelog so that only changes that matter to LIBRARY CONSUMERS appear.

Wrap the rewritten section between two marker lines so it can be extracted exactly: emit a line containing only

===BEGIN CHANGELOG===

then a blank line, then the section (starting with the version heading), then the marker line

===END CHANGELOG===

Output NOTHING outside these markers — no preamble, no commentary, no code fences, no XML tags, no system reminders, no closing remarks. The very first line of your response must be ===BEGIN CHANGELOG=== and the very last must be ===END CHANGELOG===.

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
  - Fixes for bugs introduced by another entry in this same release. The feature ships in its fixed form, so the fix is not a user-visible change relative to the previous release. Example: if "Added: starts_with operator" appears in the same section as "Fixed: escape LIKE wildcards in starts_with", drop the fix — consumers only ever see the working version. Be conservative: only drop when the fix is clearly tied to a feature/change in the same release (shared subject, shared PR thread, the fix would be nonsensical without the feature). When in doubt, keep the fix.

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

# Extract the changelog body from Claude's raw output, discarding any
# leaked commentary. We prefer the explicit ===BEGIN/END CHANGELOG===
# markers the prompt asks for: keeping only the lines between them drops
# both leading preamble (e.g. "Now I'll output the rewritten changelog:")
# and trailing closing remarks. If the markers are absent (model didn't
# follow instructions), we fall back to dropping everything before the
# first version heading (## [...]) — that still strips a leading preamble
# so we keep filtering rather than emitting the raw section. In both modes
# we also strip any <system-reminder>...</system-reminder> blocks and trim
# leading/trailing blank lines. release-plz / git-cliff expect each rendered
# section to be wrapped with one leading and one trailing blank line so
# consecutive sections in the assembled CHANGELOG.md are separated; we
# re-add those wrappers at the bottom.
BODY="$(printf '%s\n' "$OUTPUT" | awk '
  { raw[++R] = $0 }
  $0 == "===BEGIN CHANGELOG===" { hasbegin = 1 }
  $0 == "===END CHANGELOG===" { hasend = 1 }
  END {
    use_markers = (hasbegin && hasend)
    for (i = 1; i <= R; i++) {
      line = raw[i]
      if (line ~ /<system-reminder>/) { skip = 1; continue }
      if (line ~ /<\/system-reminder>/) { skip = 0; continue }
      if (skip) continue
      if (use_markers) {
        if (line == "===BEGIN CHANGELOG===") { inmarker = 1; continue }
        if (line == "===END CHANGELOG===") { inmarker = 0; continue }
        if (!inmarker) continue
      }
      if (!started && line ~ /^## \[/) started = 1
      if (!started) continue
      lines[++n] = line
    }
    s = 1; while (s <= n && lines[s] == "") s++
    e = n; while (e >= s && lines[e] == "") e--
    for (i = s; i <= e; i++) print lines[i]
  }
')"

# If preamble-stripping left nothing (e.g. the heading didn't render where
# we expected), fall back to the original input rather than emit a blank
# section.
if [ -z "$BODY" ]; then
  printf '%s' "$INPUT"
  exit 0
fi

# Collect a sorted, deduped list of `[#NNN]: <url>` definitions from every
# inline PR reference Claude kept. Then strip the URL out of each inline
# `[#NNN](<url>)` so only `[#NNN]` remains, leaving the URL to render once
# at the bottom of the section. The substitution leaves the section
# heading link `## [version](compare-url)` alone because its bracket
# content does not start with `#`.
REF_DEFS="$(printf '%s\n' "$BODY" \
  | grep -oE '\[#[0-9]+\]\([^)]+\)' \
  | sed -E 's/^(\[#[0-9]+\])\(([^)]+)\)$/\1: \2/' \
  | sort -u -t '#' -k 2 -n)"

BODY_REF="$(printf '%s' "$BODY" \
  | sed -E 's/(\[#[0-9]+\])\([^)]+\)/\1/g')"

if [ -n "$REF_DEFS" ]; then
  FINAL=$'\n'"$BODY_REF"$'\n\n'"$REF_DEFS"$'\n'
else
  FINAL=$'\n'"$BODY_REF"$'\n'
fi
printf '%s' "$FINAL" >"$CACHE_FILE"
printf '%s' "$FINAL"
