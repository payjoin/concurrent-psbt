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
  --log-revset        use the VCS's own default revset (skip default range)
  --everything        scrub all branches (jj: all() ~ root(); git: --all)

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
everything=false
use_log_revset=false

while [ $# -gt 0 ]; do
  case "$1" in
  -h | --help) usage ;;
  --log=*) vcs_mode="${1#--log=}" ;;
  --git) vcs_mode=git ;;
  --everything) everything=true ;;
  --log-revset) use_log_revset=true ;;
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

# --everything / --log-revset: validate and expand
if [ "$everything" = true ]; then
  if [ ${#log_args[@]} -gt 0 ] || [ "$use_log_revset" = true ]; then
    echo "error: --everything cannot be combined with explicit revsets or --log-revset" >&2
    exit 1
  fi
  if [ "$vcs_mode" = jj ]; then
    log_args=(-r 'all() ~ root()')
  else
    log_args=(--all)
  fi
elif [ "$use_log_revset" = true ]; then
  if [ ${#log_args[@]} -gt 0 ]; then
    echo "error: --log-revset cannot be combined with explicit revsets" >&2
    exit 1
  fi
fi

# Resolve revsets to git commit IDs
if [ "$vcs_mode" = jj ]; then
  if [ ${#log_args[@]} -gt 0 ]; then
    jj_args=(log --ignore-working-copy --no-graph -T 'commit_id ++ "\n"')
    jj_args+=("${log_args[@]}")
    mapfile -t linear < <(jj "${jj_args[@]}" 2>/dev/null | grep -vE '^0*$')
  elif [ "$use_log_revset" = true ]; then
    # Use jj's configured default log revset (no -r flag)
    mapfile -t linear < <(jj log --ignore-working-copy --no-graph -T 'commit_id ++ "\n"' 2>/dev/null | grep -vE '^0*$')
  else
    # Default: all ancestors of working copy (excluding root)
    mapfile -t linear < <(jj log --ignore-working-copy --no-graph -r 'trunk()..@' -T 'commit_id ++ "\n"' 2>/dev/null | grep -vE '^0*$')
  fi
else
  if [ ${#log_args[@]} -gt 0 ]; then
    mapfile -t linear < <(git rev-list "${log_args[@]}" 2>/dev/null)
  elif [ "$use_log_revset" = true ]; then
    mapfile -t linear < <(git rev-list HEAD 2>/dev/null)
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

# Gitignore monotonicity: check no historical commit has files that
# the tip's .gitignore would exclude (using a scratch repo for check-ignore)
gitignore_failed=()
tip_hash=${linear[0]}
if git cat-file -e "$tip_hash:.gitignore" 2>/dev/null; then
  tmpdir=$(mktemp -d)
  trap 'rm -rf "$tmpdir"' EXIT
  git show "$tip_hash:.gitignore" >"$tmpdir/.gitignore"
  git init -q "$tmpdir/repo"
  cp "$tmpdir/.gitignore" "$tmpdir/repo/.gitignore"

  echo "Checking gitignore monotonicity..."
  for hash in "${linear[@]}"; do
    leaked=$(git ls-tree -r --name-only "$hash" 2>/dev/null |
      git -C "$tmpdir/repo" check-ignore --stdin 2>/dev/null || true)
    if [ -n "$leaked" ]; then
      first=$(echo "$leaked" | head -1)
      count=$(echo "$leaked" | wc -l)
      echo "  ✗ $(fmt_commit "$hash") — $count file(s) leaked, e.g. $first"
      gitignore_failed+=("$hash")
    fi
  done
  if [ "${#gitignore_failed[@]}" -eq 0 ]; then
    echo "  all clean"
  else
    echo "${#gitignore_failed[@]} commit(s) have gitignore leaks"
  fi
fi

# Summary
n_issues=$((${#msg_failed[@]} + ${#gitignore_failed[@]}))
if [ "$n_issues" -eq 0 ]; then
  echo "All $total commits passed."
else
  echo
  echo "$n_issues issue(s):"
  for h in "${msg_failed[@]}"; do
    echo "  message: $(fmt_commit "$h")"
  done
  for h in "${gitignore_failed[@]}"; do
    echo "  gitignore: $(fmt_commit "$h")"
  done
  exit 1
fi
