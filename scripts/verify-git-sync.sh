#!/bin/sh

set -eu

fail() {
  echo "Git synchronization check failed: $1" >&2
  exit 1
}

branch=$(git symbolic-ref --quiet --short HEAD 2>/dev/null || true)
if [ -z "$branch" ]; then
  fail "HEAD is detached"
fi

if ! git diff --quiet --ignore-submodules --; then
  fail "tracked working-tree changes are present"
fi
if ! git diff --cached --quiet --ignore-submodules --; then
  fail "staged changes are present"
fi

untracked=$(git ls-files --others --exclude-standard)
if [ -n "$untracked" ]; then
  echo "$untracked" >&2
  fail "unignored files are present"
fi

remote=$(git config "branch.$branch.remote" || true)
merge_ref=$(git config "branch.$branch.merge" || true)
if [ -z "$remote" ] || [ "$remote" = "." ] || [ -z "$merge_ref" ]; then
  fail "branch $branch has no remote upstream"
fi

local_head=$(git rev-parse HEAD)
tracking_head=$(git rev-parse "@{upstream}" 2>/dev/null || true)
if [ -z "$tracking_head" ] || [ "$local_head" != "$tracking_head" ]; then
  fail "local HEAD does not match its remote-tracking ref"
fi

remote_listing=$(git ls-remote --exit-code "$remote" "$merge_ref" 2>/dev/null) ||
  fail "the configured remote branch cannot be queried"
remote_head=$(printf '%s\n' "$remote_listing" | awk 'NR == 1 { print $1 }')
if [ -z "$remote_head" ] || [ "$local_head" != "$remote_head" ]; then
  fail "local HEAD does not match the actual remote branch"
fi

echo "branch=$branch"
echo "commit=$local_head"
echo "remote=$remote"
echo "remote_ref=$merge_ref"
echo "git_sync_result=passed"
