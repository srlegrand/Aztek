#!/usr/bin/env python3
"""Opens a pull request from the patch in a "Send a patch" issue.

Run by .github/workflows/patch-to-pr.yml in a checkout of main. Reads the issue from the environment (ISSUE_BODY,
ISSUE_TITLE, ISSUE_NUMBER, ISSUE_AUTHOR, ISSUE_AUTHOR_ID), takes the patch from the issue's "The patch" section (a
dragged-in file's link, or pasted text), applies it on a branch patch/issue-N, pushes that and opens the pull request,
then says so on the issue. Nothing of the patch is run. A patch that touches .github/ is refused.

PATCH_TO_PR_DRY=1 stops before pushing, for trying it locally.
"""
import io
import os
import re
import subprocess
import sys
import urllib.request
import zipfile

REPO = os.environ.get("GITHUB_REPOSITORY", "srlegrand/Aztek")
NUMBER = os.environ["ISSUE_NUMBER"]
TITLE = os.environ.get("ISSUE_TITLE", "").strip()
AUTHOR = os.environ.get("ISSUE_AUTHOR", "someone")
AUTHOR_ID = os.environ.get("ISSUE_AUTHOR_ID", "0")
BODY = os.environ.get("ISSUE_BODY", "").replace("\r\n", "\n")
DRY = os.environ.get("PATCH_TO_PR_DRY") == "1"
BRANCH = f"patch/issue-{NUMBER}"


def run(*cmd, check=True, **kw):
    return subprocess.run(cmd, check=check, text=True, capture_output=True, **kw)


def say(text):
    """A comment on the issue, and the reason in the log."""
    print(text)
    if not DRY:
        run("gh", "issue", "comment", NUMBER, "--repo", REPO, "--body", text, check=False)


def stop(text):
    say(text + "\n\nEdit the issue with a corrected patch and this runs again. Otherwise the patch will be looked at by hand.")
    sys.exit(0)


def section(label):
    """The text under one of the form's headings."""
    m = re.search(r"^### " + re.escape(label) + r"\s*\n(.*?)(?=^### |\Z)", BODY, re.S | re.M)
    text = m.group(1).strip() if m else ""
    return "" if text == "_No response_" else text


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "aztek-patch-to-pr"})
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read(30 * 1024 * 1024 + 1)
    if len(data) > 30 * 1024 * 1024:
        stop("The attached file is larger than 30 MB.")
    return data


def patch_bytes():
    text = section("The patch")
    link = re.search(r"https://github\.com/(?:user-attachments/files/\d+|[\w.-]+/[\w.-]+/files/\d+)/[^\s)\]>\"']+", text)
    if link:
        try:
            data = fetch(link.group(0))
        except Exception as e:  # the reason goes on the issue
            stop(f"The attached file could not be downloaded ({e}).")
        if data[:2] == b"PK":
            with zipfile.ZipFile(io.BytesIO(data)) as z:
                names = [n for n in z.namelist() if n.lower().endswith((".patch", ".diff"))] or [n for n in z.namelist() if not n.endswith("/")]
                if not names:
                    stop("The zip holds no patch file.")
                data = z.read(names[0])
        return data.replace(b"\r\n", b"\n")
    # Pasted: with or without a code fence around it.
    lines = text.split("\n")
    if lines and lines[0].startswith("```"):
        lines = lines[1:]
        if lines and lines[-1].strip() == "```":
            lines = lines[:-1]
    start = next((i for i, l in enumerate(lines) if l.startswith(("diff --git ", "--- a/", "--- /dev/null", "Index: "))), None)
    if start is None:
        stop("No patch was found in the issue: drag the patch file into The patch box, or paste its text there.")
    return ("\n".join(lines[start:]).rstrip("\n") + "\n").encode()


def main():
    data = patch_bytes()
    touched = re.findall(rb"^(?:\+\+\+|---) (?:[ab]/)?(\S+)", data, re.M)
    if any(p.startswith(b".github/") for p in touched):
        stop("The patch changes files under `.github/`, which this does not take. Send that part as a pull request.")
    with open("/tmp/issue.patch", "wb") as f:
        f.write(data)
    run("git", "checkout", "-q", "-B", BRANCH)
    applied = run("git", "apply", "--index", "--whitespace=nowarn", "/tmp/issue.patch", check=False)
    if applied.returncode != 0:
        reason = (applied.stderr or "").strip().splitlines()[:8]
        stop("The patch does not apply to `main` as it is now:\n\n```\n" + "\n".join(reason) + "\n```")
    if run("git", "diff", "--cached", "--quiet", check=False).returncode == 0:
        stop("The patch changes nothing on `main`: it may already be in.")
    subject = re.sub(r"^\s*patch\s*:\s*", "", TITLE, flags=re.I).strip() or f"Patch from issue {NUMBER}"
    what, fixes = section("What was wrong and what happens now"), section("Issue it fixes")
    fixed = re.search(r"\d+", fixes)
    run("git", "-c", f"user.name={AUTHOR}", "-c", f"user.email={AUTHOR_ID}+{AUTHOR}@users.noreply.github.com", "commit", "-q", "-m", subject, "-m", f"From the patch in #{NUMBER}.")
    body = f"From the patch @{AUTHOR} sent in #{NUMBER}.\n\n{what}\n\n**How it was made:** {section('How it was made') or 'not said'}\n\n**What was checked:** {section('What you checked') or 'not said'}\n\nCloses #{NUMBER}"
    if fixed and fixed.group(0) != NUMBER:
        body += f"\nFixes #{fixed.group(0)}"
    if DRY:
        print("DRY RUN\n" + subject + "\n" + body)
        print(run("git", "show", "--stat", "--format=%an <%ae>%n%s").stdout)
        return
    run("git", "push", "-q", "--force", "origin", BRANCH)
    made = run("gh", "pr", "create", "--repo", REPO, "--base", "main", "--head", BRANCH, "--title", subject, "--body", body, check=False)
    if made.returncode == 0:
        say(f"A pull request was opened from the patch: {made.stdout.strip()}")
    elif "already exists" in (made.stderr or ""):
        say(f"The pull request for this issue was updated with the new patch (branch `{BRANCH}`).")
    else:
        say(f"The patch applies and is on the branch `{BRANCH}`, but the pull request could not be opened:\n\n```\n{(made.stderr or '').strip()[:600]}\n```")


if __name__ == "__main__":
    main()
