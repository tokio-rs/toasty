#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 <baseline.json> <candidate.json>" >&2
    exit 2
fi

baseline=$1
candidate=$2

for report in "$baseline" "$candidate"; do
    if [[ ! -f "$report" ]]; then
        echo "coverage report not found: $report" >&2
        exit 2
    fi

    if ! jq -e '.type == "llvm.coverage.json.export" and (.data | length > 0)' \
        "$report" >/dev/null; then
        echo "not an LLVM coverage JSON report: $report" >&2
        exit 2
    fi
done

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/toasty-coverage.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT

covered_regions() {
    local report=$1

    jq -r '
        .data[] |
        (reduce .files[].filename as $filename
            ({}; .[$filename] = true)) as $reported_files |
        .functions[] |
        . as $function |
        .regions[] |
        select(.[4] > 0) |
        $function.filenames[.[5]] as $filename |
        select($reported_files[$filename]) |
        [
            ($filename | sub("^.*(/|^)crates/"; "crates/")),
            .[0],
            .[1],
            .[2],
            .[3],
            .[7]
        ] |
        @tsv
    ' "$report" | LC_ALL=C sort -u
}

covered_regions "$baseline" >"$work_dir/baseline"
covered_regions "$candidate" >"$work_dir/candidate"

baseline_count=$(wc -l <"$work_dir/baseline" | tr -d ' ')
candidate_count=$(wc -l <"$work_dir/candidate" | tr -d ' ')

if [[ "$baseline_count" -eq 0 ]]; then
    echo "baseline contains no covered product regions" >&2
    exit 2
fi

LC_ALL=C comm -23 "$work_dir/baseline" "$work_dir/candidate" >"$work_dir/missing"
missing_count=$(wc -l <"$work_dir/missing" | tr -d ' ')

echo "baseline covered regions:  $baseline_count"
echo "candidate covered regions: $candidate_count"

if [[ "$missing_count" -ne 0 ]]; then
    echo "missing covered regions:   $missing_count"
    echo
    echo "first missing regions (file, start, end, kind):"
    sed -n '1,50p' "$work_dir/missing"
    exit 1
fi

echo "missing covered regions:   0"
