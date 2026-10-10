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

# ci_state prints pending, green, red, or error when GitHub can't be read.
# "No checks reported" counts as pending: right after a push they aren't
# registered yet. Any other empty answer is an error.
ci_state() {
    local json err_file
    err_file="$(mktemp)"
    json="$(gh pr checks "$pr" -R "$repo" --json name,bucket 2>"$err_file")"
    if [ -z "$json" ]; then
        if grep -qi 'no checks reported' "$err_file"; then
            echo pending
        else
            echo error
        fi
        rm -f "$err_file"
        return
    fi
    rm -f "$err_file"
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

deadline=$((SECONDS + timeout_min * 60))
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
            break
        fi
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
        break
    fi
    sleep "$interval"
done

end_head="$(head_sha)"
echo "PR #$pr in $repo, head ${end_head:0:7}"
if [ "$end_head" != "$start_head" ]; then
    echo "note: the head moved from ${start_head:0:7} while waiting"
fi

checks="$(gh pr checks "$pr" -R "$repo" --json name,bucket 2>/dev/null)"
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
