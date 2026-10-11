#!/usr/bin/env bash
# Waits once for a PR's CI to finish and its Copilot review to land, then
# prints a short verdict. Run it in the background instead of polling.
#
#   scripts/pr-wait.sh <pr-number>
#
# Env: PR_WAIT_REPO (owner/name; default: the current repo),
#      PR_WAIT_INTERVAL (seconds between polls; default 30),
#      PR_WAIT_TIMEOUT (minutes before giving up; default 45),
#      PR_WAIT_MAX_ERRORS (GitHub read failures in a row before exit 3; default 3).
#
# Exit: 0 CI green, 1 CI red, 2 timed out, 3 usage or GitHub error.
# Copilot counts as done once it is no longer a requested reviewer: either it
# reviewed, or it was never requested.
set -uo pipefail

if [ $# -ne 1 ] || ! [[ $1 =~ ^[0-9]+$ ]]; then
    echo "usage: pr-wait.sh <pr-number>" >&2
    exit 3
fi
pr="$1"
interval="${PR_WAIT_INTERVAL:-30}"
timeout_min="${PR_WAIT_TIMEOUT:-45}"
max_errors="${PR_WAIT_MAX_ERRORS:-3}"

if ! repo="${PR_WAIT_REPO:-$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null)}" || [ -z "$repo" ]; then
    echo "pr-wait: cannot tell which repository this is; set PR_WAIT_REPO" >&2
    exit 3
fi

head_sha() {
    gh pr view "$pr" -R "$repo" --json headRefOid --jq .headRefOid 2>/dev/null
}

state_dir="$(mktemp -d)" || exit 3
trap 'rm -rf "$state_dir"' EXIT

# ci_state prints pending, green, red, or error when GitHub can't be read,
# and saves the last payload it could parse to $state_dir/checks.json for the
# verdict. "No checks reported" counts as pending: right after a push they
# aren't registered yet. Any other empty or unparsable answer is an error.
ci_state() {
    local json
    json="$(gh pr checks "$pr" -R "$repo" --json name,bucket 2>"$state_dir/checks.err")"
    if [ -z "$json" ]; then
        if grep -qi 'no checks reported' "$state_dir/checks.err"; then
            echo '[]' >"$state_dir/checks.json"
            echo pending
        else
            echo error
        fi
        return
    fi
    if ! jq -e 'type == "array"' <<<"$json" >/dev/null 2>&1; then
        echo error
        return
    fi
    printf '%s\n' "$json" >"$state_dir/checks.json"
    jq -r '
        if length == 0 then "pending"
        elif any(.[]; .bucket == "pending") then "pending"
        elif any(.[]; .bucket == "fail" or .bucket == "cancel") then "red"
        else "green" end' <<<"$json" 2>/dev/null || echo error
}

# copilot_requested prints 1 while Copilot is a pending reviewer, 0 when it
# isn't, or error when GitHub can't be read.
copilot_requested() {
    local answer
    answer="$(gh api "repos/$repo/pulls/$pr/requested_reviewers" \
        --jq '[.users[].login | ascii_downcase | select(contains("copilot"))] | length | if . > 0 then 1 else 0 end' \
        2>/dev/null)"
    case "$answer" in
        0 | 1) echo "$answer" ;;
        *) echo error ;;
    esac
}

# github_error <what> — exit 3 after max_errors failed reads in a row.
errors=0
github_error() {
    errors=$((errors + 1))
    if [ "$errors" -ge "$max_errors" ]; then
        echo "pr-wait: cannot read $1 for PR #$pr in $repo ($errors tries)" >&2
        exit 3
    fi
}

start_head="$(head_sha)"
if [ -z "$start_head" ]; then
    echo "pr-wait: cannot read PR #$pr in $repo" >&2
    exit 3
fi

# wait_once polls until CI settles and Copilot is no longer requested, or the
# deadline passes; it leaves the last states in $ci and $copilot.
wait_once() {
    ci="pending"
    copilot=1
    while :; do
        ci="$(ci_state)"
        copilot="$(copilot_requested)"
        if [ "$ci" = error ]; then
            github_error "the checks"
        elif [ "$copilot" = error ]; then
            github_error "the requested reviewers"
        else
            errors=0
            if [ "$ci" != "pending" ] && [ "$copilot" = 0 ]; then
                return
            fi
        fi
        if [ "$SECONDS" -ge "$deadline" ]; then
            return
        fi
        sleep "$interval"
    done
}

# A push while waiting makes the states belong to an old head: wait again for
# the new one, within the same deadline.
deadline=$((SECONDS + timeout_min * 60))
moved_from=""
while :; do
    wait_once
    end_head="$(head_sha)"
    if [ -z "$end_head" ]; then
        echo "pr-wait: cannot read PR #$pr in $repo" >&2
        exit 3
    fi
    if [ "$end_head" = "$start_head" ]; then
        break
    fi
    moved_from="${moved_from:-$start_head}"
    start_head="$end_head"
done

echo "PR #$pr in $repo, head ${end_head:0:7}"
if [ -n "$moved_from" ]; then
    echo "note: the head moved from ${moved_from:0:7} while waiting; this verdict is for ${end_head:0:7}"
fi

checks="$(cat "$state_dir/checks.json" 2>/dev/null)"
[ -n "$checks" ] || checks='[]'
case "$ci" in
    green)
        jq -r '"CI: green, \(map(select(.bucket == "pass")) | length) passed, \(map(select(.bucket == "skipping")) | length) skipped"' <<<"$checks"
        ;;
    red)
        jq -r '"CI: red, failed: \(map(select(.bucket == "fail" or .bucket == "cancel") | .name) | join(", "))"' <<<"$checks"
        ;;
    pending)
        jq -r '"CI: still running at the timeout: \(map(select(.bucket == "pending") | .name) | join(", "))"' <<<"$checks"
        ;;
    *)
        echo "CI: could not read the checks"
        ;;
esac

if [ "$copilot" = error ]; then
    echo "Copilot: could not read the requested reviewers"
    exit 3
elif [ "$copilot" != 0 ]; then
    echo "Copilot: still requested at the timeout"
elif ! reviews="$(gh api "repos/$repo/pulls/$pr/reviews" --paginate \
    --jq '.[] | select(.user.login | ascii_downcase | contains("copilot")) | "\(.id) \(.commit_id)"' 2>/dev/null)"; then
    echo "Copilot: could not read the reviews"
    exit 3
elif [ -z "$reviews" ]; then
    echo "Copilot: not requested, no review"
else
    read -r review_id review_commit <<<"$(tail -n 1 <<<"$reviews")"
    if counts="$(gh api "repos/$repo/pulls/$pr/reviews/$review_id/comments" --paginate --jq 'length' 2>/dev/null)"; then
        comments="$(awk '{ n += $1 } END { print n + 0 }' <<<"$counts")"
        echo "Copilot: reviewed ${review_commit:0:7}, $comments inline comment(s)"
    else
        echo "Copilot: reviewed ${review_commit:0:7}, inline comments unreadable"
    fi
fi

case "$ci" in
    green) [ "$copilot" = 0 ] && exit 0 || exit 2 ;;
    red) exit 1 ;;
    pending) exit 2 ;;
    *) exit 3 ;;
esac
