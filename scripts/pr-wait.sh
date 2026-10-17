#!/usr/bin/env bash
# Waits once for a PR's CI to finish and its Copilot and CodeRabbit reviews to
# land, then prints a short verdict. Run it in the background instead of polling.
#
#   scripts/pr-wait.sh <pr-number>
#
# Env: PR_WAIT_REPO (owner/name; default: `gh repo view`, else the GitHub URL
#        of jj's `origin` remote, so a jj workspace without .git works),
#      PR_WAIT_COPILOT (0: don't wait for Copilot; default 1; a PR whose
#        base isn't main never waits, since Copilot isn't requested there),
#      PR_WAIT_CODERABBIT (0: don't wait for CodeRabbit; default 1; a PR whose
#        base isn't main never waits for it either),
#      PR_WAIT_INTERVAL (seconds between polls; default 30),
#      PR_WAIT_TIMEOUT (minutes before giving up on CI, Copilot and CodeRabbit;
#        default 45),
#      PR_WAIT_MAX_ERRORS (GitHub read failures in a row before exit 3; default 3).
#
# Exit: 0 CI green and Copilot and CodeRabbit reviewed the head, 1 CI red,
# 2 timed out, 3 usage or GitHub error, 4 CI green but Copilot unable to
# review the head (its quota notice, say): re-request it later, or go on
# without it, 5 CI green but CodeRabbit didn't review the head (its "Review
# skipped", a rate limit or a pause): tell the Engineer.
# Copilot counts as done only once it has a review whose commit is the PR's
# head; leaving the requested reviewers isn't enough, since that happens when
# its review starts. A review whose body says Copilot was unable (or wasn't
# able) to review is a notice, not a review.
# CodeRabbit counts as done only once it has reviewed the head: a review by
# coderabbitai[bot] whose commit is the head and whose body posts actionable
# comments, or its summary comment saying the recent review, up to the head,
# generated none. Its CodeRabbit status check alone isn't enough: it turns
# success ("Review completed", even) on heads CodeRabbit skipped.
set -uo pipefail

if [ $# -ne 1 ] || ! [[ $1 =~ ^[0-9]+$ ]]; then
    echo "usage: pr-wait.sh <pr-number>" >&2
    exit 3
fi
pr="$1"
interval="${PR_WAIT_INTERVAL:-30}"
timeout_min="${PR_WAIT_TIMEOUT:-45}"
max_errors="${PR_WAIT_MAX_ERRORS:-3}"

copilot_wait="${PR_WAIT_COPILOT:-1}"
copilot_skip_reason="PR_WAIT_COPILOT=0"
coderabbit_wait="${PR_WAIT_CODERABBIT:-1}"
coderabbit_skip_reason="PR_WAIT_CODERABBIT=0"

# detect_repo prints owner/name: PR_WAIT_REPO, else what gh reads from git,
# else the GitHub URL of jj's `origin` remote (or its only remote).
detect_repo() {
    local found url
    if [ -n "${PR_WAIT_REPO:-}" ]; then
        echo "$PR_WAIT_REPO"
        return
    fi
    if found="$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null)" && [ -n "$found" ]; then
        echo "$found"
        return
    fi
    url="$(jj git remote list 2>/dev/null |
        awk '$1 == "origin" { print $2; done = 1; exit } { n++; only = $2 } END { if (!done && n == 1) print only }')"
    url="${url%.git}"
    case "$url" in
        git@github.com:*) found="${url#git@github.com:}" ;;
        ssh://git@github.com/*) found="${url#ssh://git@github.com/}" ;;
        https://github.com/*) found="${url#https://github.com/}" ;;
        *) return 1 ;;
    esac
    [[ $found =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || return 1
    echo "$found"
}

if ! repo="$(detect_repo)" || [ -z "$repo" ]; then
    echo "pr-wait: cannot tell which repository this is; set PR_WAIT_REPO" >&2
    exit 3
fi

head_sha() {
    gh pr view "$pr" -R "$repo" --json headRefOid --jq .headRefOid 2>/dev/null
}

# Copilot is requested only on PRs into main, and only those wait for
# CodeRabbit; a slice PR waits for neither.
if [ "$copilot_wait" != 0 ] || [ "$coderabbit_wait" != 0 ]; then
    if ! base="$(gh pr view "$pr" -R "$repo" --json baseRefName --jq .baseRefName 2>/dev/null)" || [ -z "$base" ]; then
        echo "pr-wait: cannot read the base of PR #$pr in $repo" >&2
        exit 3
    fi
    if [ "$base" != main ]; then
        if [ "$copilot_wait" != 0 ]; then
            copilot_wait=0
            copilot_skip_reason="a PR into $base"
        fi
        if [ "$coderabbit_wait" != 0 ]; then
            coderabbit_wait=0
            coderabbit_skip_reason="a PR into $base"
        fi
    fi
fi

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

# copilot_reviews prints one tab-separated line per Copilot review, oldest
# first: id, commit, 1 when its body says Copilot was unable or wasn't able
# to review (else 0), and that body on one line. Fails when GitHub can't be read.
copilot_reviews() {
    gh api "repos/$repo/pulls/$pr/reviews" --paginate \
        --jq '.[] | select(.user.login | ascii_downcase | contains("copilot"))
            | (.body // "") as $body
            | [(.id | tostring), .commit_id,
               (if ($body | test("(unable|wasn.t able|was not able) to review"; "i")) then "1" else "0" end),
               ($body | gsub("[\\t\\r\\n]+"; " ") | .[0:300])]
            | @tsv' 2>/dev/null
}

# copilot_state <head> prints reviewed once a Copilot review's commit is
# <head>, unavailable when Copilot's only answers for <head> are "unable to
# review" notices, waiting until either, skipped under PR_WAIT_COPILOT=0, or
# error when GitHub can't be read.
copilot_state() {
    local reviews
    if [ "$copilot_wait" = 0 ]; then
        echo skipped
        return
    fi
    if ! reviews="$(copilot_reviews)"; then
        echo error
        return
    fi
    awk -F '\t' -v head="$1" '
        $2 == head && $3 == 0 { reviewed = 1 }
        $2 == head && $3 == 1 { notice = 1 }
        END { print reviewed ? "reviewed" : notice ? "unavailable" : "waiting" }' <<<"$reviews"
}

# CodeRabbit is matched by its exact login: GitHub usernames can't hold "[",
# so only the CodeRabbit App posts as coderabbitai[bot], while anyone can
# comment on a public PR.

# coderabbit_reviews prints one tab-separated line per CodeRabbit review that
# posts actionable comments, oldest first: commit and the number posted. The
# empty-body reviews it makes when it replies in a comment thread don't
# count. Fails when GitHub can't be read.
coderabbit_reviews() {
    gh api "repos/$repo/pulls/$pr/reviews" --paginate \
        --jq '.[] | select(.user.login == "coderabbitai[bot]")
            | ((.body // "") | capture("Actionable comments posted: (?<n>[0-9]+)")) as $m
            | [.commit_id, $m.n]
            | @tsv' 2>/dev/null
}

# coderabbit_comments prints the bodies of CodeRabbit's issue comments, every
# line prefixed with "|" and each body followed by a "--" line, so a reader
# can tell where one comment ends. Fails when GitHub can't be read.
coderabbit_comments() {
    gh api "repos/$repo/issues/$pr/comments" --paginate \
        --jq '.[] | select(.user.login == "coderabbitai[bot]")
            | (((.body // "") | gsub("\r"; "") | split("\n") | .[] | "|" + .), "--")' 2>/dev/null
}

# coderabbit_status <sha> prints CodeRabbit's latest status on <sha>, tab-
# separated: state and description; nothing when it has none. The combined
# status endpoint drops the creator, so this reads the list, newest first.
# Fails when GitHub can't be read.
coderabbit_status() {
    gh api "repos/$repo/commits/$1/statuses?per_page=100" \
        --jq 'first(.[] | select(.context == "CodeRabbit" and .creator.login == "coderabbitai[bot]"))
            | [.state, ((.description // "") | gsub("[\\t\\r\\n]+"; " "))]
            | @tsv' 2>/dev/null
}

# coderabbit_summary <head> reads coderabbit_comments and prints, tab-
# separated: 1 when a comment says the recent review, between some commit and
# <head>, generated no actionable comments (else 0), and the reason of the
# last "Review skipped" block (one line, at most 200 characters). Only text
# outside CodeRabbit's "skip review", "review in progress" and other marked
# blocks counts, since those name the head of a review that was skipped or
# hasn't finished; the "summarize" marker opens no block. Each comment is read
# on its own, so a block left open in one can't swallow another.
coderabbit_summary() {
    awk -v head="$1" '
        function ends(s, t) { return length(s) >= length(t) && substr(s, length(s) - length(t) + 1) == t }
        function end_comment() {
            if (no_actionable && between) reviewed = 1
            no_actionable = between = want_reason = 0
            block = ""
        }
        BEGIN {
            start = "<!-- This is an auto-generated comment: "
            stop = "<!-- end of auto-generated comment: "
        }
        substr($0, 1, 1) != "|" { end_comment(); next }
        {
            line = substr($0, 2)
            sub(/[ \t]+$/, "", line)
            if ((i = index(line, start))) {
                kind = substr(line, i + length(start))
                if ((j = index(kind, " by coderabbit.ai -->")) && substr(kind, 1, j - 1) != "summarize")
                    block = substr(kind, 1, j - 1)
                next
            }
            if (index(line, stop)) {
                block = ""
                want_reason = 0
                next
            }
            if (block == "skip review") {
                if (want_reason) {
                    s = line
                    sub(/^[> \t]+/, "", s)
                    gsub(/\t/, " ", s)
                    if (s != "") {
                        skip = substr(s, 1, 200)
                        want_reason = 0
                    }
                }
                if (index(line, "## Review skipped")) want_reason = 1
                next
            }
            if (block != "") next
            if (index(line, "No actionable comments were generated in the recent review")) no_actionable = 1
            if (index(line, "between ") && (ends(line, " and " head) || ends(line, " and " head "."))) between = 1
        }
        END {
            end_comment()
            printf "%d\t%s\n", reviewed, skip
        }'
}

# coderabbit_state <head> prints reviewed once CodeRabbit has reviewed
# <head> (see the header), unreviewed when its status on <head> is there and
# not pending but no review of <head> is, waiting until either, skipped when
# not waited for, or error when GitHub can't be read. It saves what the
# verdict needs under $state_dir: coderabbit-actionable (the number posted,
# or "none"), coderabbit-status (state and description) and coderabbit-skip
# (the "Review skipped" reason).
coderabbit_state() {
    local sha="$1" reviews comments status actionable summary
    if [ "$coderabbit_wait" = 0 ]; then
        echo skipped
        return
    fi
    if ! reviews="$(coderabbit_reviews)" || ! comments="$(coderabbit_comments)" ||
        ! status="$(coderabbit_status "$sha")"; then
        echo error
        return
    fi
    actionable="$(awk -F '\t' -v head="$sha" '$1 == head { n = $2 } END { print n }' <<<"$reviews")"
    summary="$(coderabbit_summary "$sha" <<<"$comments")"
    printf '%s\n' "$status" >"$state_dir/coderabbit-status"
    printf '%s\n' "${summary#*$'\t'}" >"$state_dir/coderabbit-skip"
    if [ -n "$actionable" ]; then
        echo "$actionable" >"$state_dir/coderabbit-actionable"
        echo reviewed
    elif [ "${summary%%$'\t'*}" = 1 ]; then
        echo none >"$state_dir/coderabbit-actionable"
        echo reviewed
    elif [ -n "$status" ] && [ "${status%%$'\t'*}" != pending ]; then
        echo unreviewed
    else
        echo waiting
    fi
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

# wait_once polls until CI settles and Copilot and CodeRabbit have reviewed
# $start_head, or the deadline passes; it leaves the last states in $ci,
# $copilot and $coderabbit.
# CodeRabbit's status can turn success a few seconds before it edits its
# summary comment with the review, and a "Review skipped" block from an
# earlier head can stand until then; so a settled status with no review of
# the head becomes final (unavailable) only on the second poll in a row, or
# at the deadline. The count starts afresh for each head.
wait_once() {
    local unreviewed_polls=0
    ci="pending"
    copilot=waiting
    coderabbit=waiting
    while :; do
        ci="$(ci_state)"
        copilot="$(copilot_state "$start_head")"
        coderabbit="$(coderabbit_state "$start_head")"
        case "$coderabbit" in
            unreviewed)
                unreviewed_polls=$((unreviewed_polls + 1))
                if [ "$unreviewed_polls" -ge 2 ] || [ "$SECONDS" -ge "$deadline" ]; then
                    coderabbit=unavailable
                else
                    coderabbit=waiting
                fi
                ;;
            error) ;;
            *) unreviewed_polls=0 ;;
        esac
        if [ "$ci" = error ]; then
            github_error "the checks"
        elif [ "$copilot" = error ]; then
            github_error "the Copilot reviews"
        elif [ "$coderabbit" = error ]; then
            github_error "the CodeRabbit review"
        else
            errors=0
            if [ "$ci" != "pending" ] && [ "$copilot" != waiting ] && [ "$coderabbit" != waiting ]; then
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

case "$copilot" in
    skipped)
        echo "Copilot: not waited for ($copilot_skip_reason)"
        ;;
    error)
        echo "Copilot: could not read the reviews"
        exit 3
        ;;
    *)
        if ! reviews="$(copilot_reviews)"; then
            echo "Copilot: could not read the reviews"
            exit 3
        fi
        last_commit="$(awk -F '\t' '$3 == 0 { c = $2 } END { print c }' <<<"$reviews")"
        if [ "$copilot" = unavailable ]; then
            notice="$(awk -F '\t' -v head="$end_head" '$2 == head && $3 == 1 { b = $4 } END { print b }' <<<"$reviews")"
            # The reason after "because", else the notice's first sentence.
            reason="${notice#*because }"
            reason="${reason%%. *}"
            reason="${reason%.}"
            echo "Copilot: unavailable ($reason)"
        elif [ "$copilot" = reviewed ]; then
            review_id="$(awk -F '\t' -v head="$end_head" '$2 == head && $3 == 0 { id = $1 } END { print id }' <<<"$reviews")"
            if counts="$(gh api "repos/$repo/pulls/$pr/reviews/$review_id/comments" --paginate --jq 'length' 2>/dev/null)"; then
                comments="$(awk '{ n += $1 } END { print n + 0 }' <<<"$counts")"
                echo "Copilot: reviewed ${end_head:0:7}, $comments inline comment(s)"
            else
                echo "Copilot: reviewed ${end_head:0:7}, inline comments unreadable"
            fi
        elif [ -n "$last_commit" ]; then
            echo "Copilot: no review of ${end_head:0:7} by the timeout (last reviewed ${last_commit:0:7})"
        else
            echo "Copilot: no review of ${end_head:0:7} by the timeout (none yet)"
        fi
        ;;
esac

case "$coderabbit" in
    skipped)
        echo "CodeRabbit: not waited for ($coderabbit_skip_reason)"
        ;;
    error)
        echo "CodeRabbit: could not read the review"
        exit 3
        ;;
    reviewed)
        actionable="$(cat "$state_dir/coderabbit-actionable" 2>/dev/null)"
        if [ "$actionable" = none ]; then
            echo "CodeRabbit: reviewed ${end_head:0:7}, no actionable comments"
        else
            echo "CodeRabbit: reviewed ${end_head:0:7}, $actionable actionable comment(s)"
        fi
        ;;
    *)
        cr_state=""
        cr_desc=""
        IFS=$'\t' read -r cr_state cr_desc 2>/dev/null <"$state_dir/coderabbit-status"
        cr_desc="${cr_desc:-$cr_state}"
        if [ "$coderabbit" = unavailable ]; then
            echo "CodeRabbit: no review of ${end_head:0:7} ($cr_desc)"
            skip="$(cat "$state_dir/coderabbit-skip" 2>/dev/null)"
            if [ -n "$skip" ]; then
                echo "CodeRabbit: Review skipped: $skip"
            fi
        elif [ -n "${cr_state:-}" ]; then
            echo "CodeRabbit: no review of ${end_head:0:7} by the timeout (status: $cr_desc)"
        else
            echo "CodeRabbit: no review of ${end_head:0:7} by the timeout (no CodeRabbit status yet)"
        fi
        ;;
esac

case "$ci" in
    green)
        if [ "$copilot" = waiting ] || [ "$coderabbit" = waiting ]; then
            exit 2
        fi
        case "$coderabbit:$copilot" in
            unavailable:*) exit 5 ;;
            *:unavailable) exit 4 ;;
            *) exit 0 ;;
        esac
        ;;
    red) exit 1 ;;
    pending) exit 2 ;;
    *) exit 3 ;;
esac
