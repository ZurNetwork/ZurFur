#!/usr/bin/env python3
"""Boardroom runner for fork 001 — SCRUTINIZE mode. Seats: GPT + Gemini. Claude judges (odd fork).

Usage: run.py S0 | S1 | S2 | S3 [--added added-options.md]
Keys are read from ~/.zshrc (they are not exported there) and never printed.
Each seat's prompt prefix is byte-identical across every call; only the phase suffix is appended.
"""
import json, os, re, sys, threading, time, urllib.request, urllib.error

DIR = os.path.dirname(os.path.abspath(__file__))
BRIEF = open(os.path.join(DIR, "brief.md"), encoding="utf-8").read()
assert "<sealed>" not in BRIEF, "sealed content leaked into the seat-facing brief"

ROLE = (
    "You are one independent seat on a multi-model design-review board scrutinizing a proposal brief "
    "for Zurfur (a Rust, AT-Protocol-native art-commission platform). Be blunt, specific and evidenced; "
    "disagreement is the product, cheerleading is worthless. You see other seats' output only when a "
    "phase explicitly includes it. Never invent facts about the codebase beyond the context block; say "
    "UNVERIFIABLE instead. Keep every answer under 2000 tokens.\n\n=== BRIEF ===\n\n"
)
PREFIX = ROLE + BRIEF + "\n\n=== PHASE INSTRUCTION ===\n\n"

PHASES = {
    "S0": (
        "PHASE S0 — VERIFICATION. For each enumerated claim 1–14 give exactly one verdict: CONFIRMED, "
        "WRONG (with source and the corrected fact), or UNVERIFIABLE. Repo-internal claims: verify against "
        "the context block only. External claims (5, 12, and the PLC-directory behaviour assumed in 10): "
        "use your own web search, at most 5 fetches, and cite the URL per verdict. Do not argue the options "
        "yet. Format: a numbered list, one claim per line, verdict first."
    ),
    "S1": (
        "PHASE S1 — ATTACK (isolated). For EACH option A–E give: (a) the strongest case AGAINST it, (b) the "
        "strongest case FOR it, (c) one concrete failure scenario (state/inputs → wrong outcome). Use claims "
        "10 and 13 in their corrected form. MANDATORY at the end: name at least one option or consideration "
        "MISSING from the brief (attack the framing), or state explicitly that the option space is complete "
        "and why. Label any new option F, G, … with a one-line definition."
    ),
    "S2": (
        "PHASE S2 — REBUTTAL (one round). Below is the OTHER seat's S1 output. Attack the weakest "
        "load-bearing claims in it — name the claim, say why it is wrong or overstated, cite the brief or a "
        "source. State explicitly which of its points you CONCEDE. Do not restate your own S1.\n\n"
        "=== OTHER SEAT'S S1 ===\n\n"
    ),
    "S3": (
        "PHASE S3 — INDEPENDENT RANKING. Rank the FULL option set — A–E plus every board-added option listed "
        "below — from best to worst, one line of reason per rank. Combinations (e.g. 'D + B') are allowed "
        "as a ranked entry if you define them. Finish with one paragraph: what single piece of evidence "
        "would flip your #1.\n\n=== BOARD-ADDED OPTIONS (from S1) ===\n\n"
    ),
}


def load_keys():
    keys = {}
    pat = re.compile(r'^\s*(?:export\s+)?(OPENAI_API_KEY|GEMINI_API_KEY)=["\']?([^"\']+)["\']?\s*$')
    with open(os.path.expanduser("~/.zshrc"), encoding="utf-8") as f:
        for line in f:
            m = pat.match(line)
            if m:
                keys[m.group(1)] = m.group(2)
    for k in ("OPENAI_API_KEY", "GEMINI_API_KEY"):
        if k not in keys:
            sys.exit(f"missing {k} — refusing to mock a seat")
    return keys


def post(url, headers, body, timeout=600):
    req = urllib.request.Request(url, data=json.dumps(body).encode(), headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{url} -> HTTP {e.code}: {e.read().decode()[:800]}")


def gpt(prompt, key, usage):
    body = {
        "model": "gpt-5.6-sol",
        "input": [{"role": "user", "content": prompt}],
        "tools": [{"type": "web_search"}],
        "max_output_tokens": 12000,
        "reasoning": {"effort": "medium"},
    }
    r = post("https://api.openai.com/v1/responses",
             {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}, body)
    text = []
    for item in r.get("output", []):
        if item.get("type") == "message":
            for c in item.get("content", []):
                if c.get("type") == "output_text":
                    text.append(c["text"])
    u = r.get("usage", {})
    usage["gpt"] = usage.get("gpt", 0) + u.get("input_tokens", 0) + u.get("output_tokens", 0)
    if r.get("status") == "incomplete":
        text.append(f"\n\n[truncated: {r.get('incomplete_details')}]")
    return "\n".join(text) or f"[empty response: {json.dumps(r)[:500]}]"


def gemini(prompt, key, usage, max_out=12000):
    body = {
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
        "tools": [{"google_search": {}}],
        "generationConfig": {"maxOutputTokens": max_out},
    }
    r = post("https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-pro-preview:generateContent",
             {"x-goog-api-key": key, "Content-Type": "application/json"}, body)
    cand = (r.get("candidates") or [{}])[0]
    parts = cand.get("content", {}).get("parts", [])
    text = "".join(p.get("text", "") for p in parts if not p.get("thought"))
    u = r.get("usageMetadata", {})
    usage["gemini"] = usage.get("gemini", 0) + u.get("totalTokenCount", 0)
    if cand.get("finishReason") == "MAX_TOKENS":
        text += "\n\n[truncated at MAX_TOKENS]"
    return text or f"[empty response: {json.dumps(r)[:500]}]"


def main():
    phase = sys.argv[1]
    added = ""
    if "--added" in sys.argv:
        added = open(sys.argv[sys.argv.index("--added") + 1], encoding="utf-8").read()
    keys = load_keys()
    usage_path = os.path.join(DIR, "usage.json")
    usage = json.load(open(usage_path)) if os.path.exists(usage_path) else {}
    seats = {"gpt": lambda p: gpt(p, keys["OPENAI_API_KEY"], usage),
             "gemini": lambda p: gemini(p, keys["GEMINI_API_KEY"], usage)}
    other = {"gpt": "gemini", "gemini": "gpt"}

    def suffix(seat):
        s = PHASES[phase]
        if phase == "S2":
            s += open(os.path.join(DIR, f"S1-{other[seat]}.md"), encoding="utf-8").read()
        if phase == "S3":
            s += added or "(none)"
            # Re-supply only what this seat has already seen (stateless calls):
            # its own S1 + S2 and the other seat's S1 — never the other seat's S2 or ranking.
            for label, path in (
                ("YOUR OWN S1", f"S1-{seat}.md"),
                ("THE OTHER SEAT'S S1 (as shown to you in S2)", f"S1-{other[seat]}.md"),
                ("YOUR OWN S2 REBUTTAL", f"S2-{seat}.md"),
            ):
                s += f"\n\n=== {label} ===\n\n" + open(os.path.join(DIR, path), encoding="utf-8").read()
        return s

    results, errors = {}, {}

    def work(seat):
        t0 = time.time()
        try:
            results[seat] = seats[seat](PREFIX + suffix(seat))
        except Exception as e:  # noqa: BLE001
            errors[seat] = str(e)
        print(f"{seat} {phase} done in {time.time()-t0:.0f}s", file=sys.stderr)

    threads = [threading.Thread(target=work, args=(s,)) for s in seats]
    for t in threads: t.start()
    for t in threads: t.join()
    for seat, text in results.items():
        with open(os.path.join(DIR, f"{phase}-{seat}.md"), "w", encoding="utf-8") as f:
            f.write(text)
    json.dump(usage, open(usage_path, "w"))
    if errors:
        for s, e in errors.items():
            print(f"ERROR {s}: {e}", file=sys.stderr)
        sys.exit(1)
    print(json.dumps({"phase": phase, "seats": list(results), "usage": usage}))


if __name__ == "__main__":
    main()
