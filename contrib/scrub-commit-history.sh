#!/usr/bin/env bash
# scrub-commit-history — verify commit hygiene across history
#
# Walks commit history and checks each commit for unresolved work-item
# markers in messages (TODO, FIXME, WIP, fixup!, squash!).
set -euo pipefail

usage() {
  cat >&2 <<EOF
Usage: ${0##*/} [options] [-- <revset-args>...]

Options:
  -h, --help          show this help
  --log=jj|git        VCS for revset resolution (default: jj if available, else git)
  --git               alias for --log=git

Extra arguments are passed through as-is to jj log or git rev-list.
A single bookmark/branch name or commit hash works in both modes.

Revset defaults (when no extra arguments given):
  jj mode:   -r 'trunk()..@' (ancestors of @ since trunk)
  git mode:  origin/main..HEAD (or all commits if no remote)
EOF
  exit 1
}

log_args=()
vcs_mode=auto

while [ $# -gt 0 ]; do
  case "$1" in
  -h | --help) usage ;;
  --log=*) vcs_mode="${1#--log=}" ;;
  --git) vcs_mode=git ;;
  *) log_args+=("$1") ;;
  esac
  shift
done

repo_root=$(git rev-parse --show-toplevel)

# Resolve VCS mode
if [ "$vcs_mode" = auto ]; then
  if [ -d "$repo_root/.jj" ]; then
    vcs_mode=jj
  else
    vcs_mode=git
  fi
fi
case "$vcs_mode" in
jj | git) ;;
*)
  echo "error: --log expects jj or git (got '$vcs_mode')" >&2
  exit 1
  ;;
esac

# Resolve revsets to git commit IDs
if [ "$vcs_mode" = jj ]; then
  if [ ${#log_args[@]} -gt 0 ]; then
    jj_args=(log --ignore-working-copy --no-graph -T 'commit_id ++ "\n"')
    jj_args+=("${log_args[@]}")
    mapfile -t linear < <(jj "${jj_args[@]}" 2>/dev/null | grep -vE '^0*$')
  else
    # Default: all ancestors of working copy (excluding root)
    mapfile -t linear < <(jj log --ignore-working-copy --no-graph -r 'trunk()..@' -T 'commit_id ++ "\n"' 2>/dev/null | grep -vE '^0*$')
  fi
else
  if [ ${#log_args[@]} -gt 0 ]; then
    mapfile -t linear < <(git rev-list "${log_args[@]}" 2>/dev/null)
  else
    merge_base=$(git merge-base HEAD origin/main 2>/dev/null || true)
    if [ -n "$merge_base" ]; then
      mapfile -t linear < <(git rev-list "$merge_base..HEAD" 2>/dev/null)
    else
      mapfile -t linear < <(git rev-list HEAD 2>/dev/null)
    fi
  fi
fi

total=${#linear[@]}
if [ "$total" -eq 0 ]; then
  echo "No commits in range."
  exit 0
fi

fmt_commit() {
  if [ "$vcs_mode" = jj ]; then
    jj log --ignore-working-copy --no-graph -r "$1" \
      -T 'change_id.shortest(7) ++ " " ++ commit_id.shortest(7) ++ " " ++ description.first_line()' \
      2>/dev/null || git log -1 --format='%h %s' "$1"
  else
    git log -1 --format='%h %s' "$1"
  fi
}

# Check each commit message for unresolved work-item markers
msg_failed=()
echo "Checking commit messages..."
for hash in "${linear[@]}"; do
  if git log -1 --format='%B' "$hash" | grep -qE '^\s*[#\[]*\s*(TODO|FIXME|WIP)\b|\bfixup! |\bsquash! '; then
    echo "  ✗ $(fmt_commit "$hash")"
    msg_failed+=("$hash")
  fi
done
if [ "${#msg_failed[@]}" -gt 0 ]; then
  echo "${#msg_failed[@]} commit(s) have unresolved work items"
else
  echo "  all clean"
fi

# Summary
if [ "${#msg_failed[@]}" -eq 0 ]; then
  echo "All $total commits passed."
else
  echo
  echo "${#msg_failed[@]} failure(s):"
  for h in "${msg_failed[@]}"; do
    echo "  message: $(fmt_commit "$h")"
  done
  exit 1
fi
