#!/usr/bin/env python3
"""
Generate a "Keep a Changelog"-style JSON document from GitHub Release metadata.

Intended usage in GitHub Actions:
  python3 .github/scripts/release_notes_to_kac_json.py > payload.json

Inputs (env vars; all optional but recommended):
  - RELEASE_TAG                 e.g. "v1.2.3" (preferred)
  - RELEASE_VERSION             e.g. "1.2.3"  (optional; derived from tag if missing)
  - RELEASE_PUBLISHED_AT        RFC3339 timestamp from GitHub release event
  - RELEASE_DATE                alternative date field if you prefer your own
  - RELEASE_BODY                GitHub release body / notes (markdown text)
  - SOURCE_REPO                 "owner/repo" (falls back to GITHUB_REPOSITORY)

Alternatively, provide fields via CLI flags:
  --tag, --version, --date, --body, --source-repo

Output JSON shape:
{
  "version": "1.2.3",
  "tag": "v1.2.3",
  "date": "...",
  "source_repo": "kellnr/kellnr",
  "keep_a_changelog": {
    "Added": [...],
    "Changed": [...],
    "Deprecated": [...],
    "Removed": [...],
    "Fixed": [...],
    "Security": [...],
    "Other": [...]
  }
}

Notes:
- This is a heuristic mapper. It works best when release notes are either:
  - a bullet list of conventional-commit-like lines, or
  - a plain list of conventional commits
- It does not attempt to parse full "Keep a Changelog" markdown sections.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from typing import Dict, List, Tuple

KAC_SECTIONS: Tuple[str, ...] = (
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
    "Other",
)


def _env(name: str) -> str:
    return (os.environ.get(name) or "").strip()


def _derive_version(tag: str, version: str) -> str:
    if version:
        return version
    if tag.startswith("v") and len(tag) > 1:
        return tag[1:]
    return tag


def _init_payload(version: str, tag: str, date: str, source_repo: str) -> Dict:
    return {
        "version": version,
        "tag": tag,
        "date": date,
        "source_repo": source_repo,
        "keep_a_changelog": {k: [] for k in KAC_SECTIONS},
    }


def _is_heading(line: str) -> bool:
    return line.lstrip().startswith("#")


def _clean_line(line: str) -> str:
    s = line.strip()
    s = re.sub(r"^[-*]\s+", "", s)  # bullets
    s = re.sub(r"^\d+\.\s+", "", s)  # numbered lists
    return s.strip()


# Example supported lines:
# * ci: Add pipeline ... by @secana in https://github.com/kellnr/kellnr/pull/904
# * build(deps-dev): bump globals ... by @dependabot[bot] in https://github.com/.../pull/925
_RELEASE_NOTE_ITEM_RE = re.compile(
    r"^(?P<subject>.+?)\s+by\s+@(?P<author>[^\s]+)\s+in\s+(?P<url>https?://\S+)$",
    re.IGNORECASE,
)

_CONVENTIONAL_TYPE_SCOPE_RE = re.compile(
    r"^(?P<type>feat|fix|perf|refactor|docs|style|test|build|ci|chore|revert)"
    r"(?:\((?P<scope>[^)]+)\))?"  # optional scope
    r"(?P<breaking>!)?:\s+"  # optional breaking marker
    r"(?P<title>.+)$",
    re.IGNORECASE,
)


def _parse_release_note_item(line: str) -> Dict[str, str] | None:
    """Parse a GitHub release-note bullet line into parts.

    Returns None when the line doesn't match the expected pattern.
    """
    m = _RELEASE_NOTE_ITEM_RE.match(line.strip())
    if not m:
        return None

    subject = m.group("subject").strip()
    author = m.group("author").strip()
    url = m.group("url").strip()

    out: Dict[str, str] = {
        "subject": subject,
        "author": author,
        "url": url,
    }

    cm = _CONVENTIONAL_TYPE_SCOPE_RE.match(subject)
    if cm:
        out["type"] = (cm.group("type") or "").lower()
        out["scope"] = (cm.group("scope") or "").lower()
        out["title"] = (cm.group("title") or "").strip()

    pr = re.search(r"/pull/(\d+)(?:\D|$)", url)
    if pr:
        out["pr"] = pr.group(1)

    return out


def _is_dependabot_deps_entry(parsed: Dict[str, str]) -> bool:
    """True for Dependabot dependency update entries we want to omit."""
    author = (parsed.get("author") or "").lower()
    typ = (parsed.get("type") or "").lower()
    scope = (parsed.get("scope") or "").lower()

    if not author.startswith("dependabot"):
        return False

    # Omit only dependency bump entries (build(deps...), build(deps-dev...), ...)
    if typ == "build" and scope.startswith("deps"):
        return True

    return False



_CONVENTIONAL_PREFIX_RE = re.compile(
    r"^(feat|fix|perf|refactor|docs|style|test|build|ci|chore|revert)(\(.+\))?!?:\s+\S+",
    re.IGNORECASE,
)


def _looks_like_conventional_commit(line: str) -> bool:
    return _CONVENTIONAL_PREFIX_RE.search(line.strip()) is not None


def _bucket_for(line: str) -> str:
    """
    Map a line to a Keep a Changelog section.

    Priority:
    - Explicit security/CVE markers
    - Conventional commit type
    - Breaking change markers / keywords
    - Keyword heuristics
    """
    lower_line = line.lower().strip()

    # security signals
    if "security" in lower_line or "cve-" in lower_line:
        return "Security"

    # conventional commits
    if re.search(r"(^|\W)feat(\(|:)", lower_line):
        return "Added"
    if re.search(r"(^|\W)fix(\(|:)", lower_line):
        return "Fixed"
    if re.search(r"(^|\W)perf(\(|:)", lower_line):
        return "Changed"
    if re.search(r"(^|\W)refactor(\(|:)", lower_line):
        return "Changed"
    if re.search(r"(^|\W)docs(\(|:)", lower_line):
        return "Changed"
    if re.search(r"(^|\W)style(\(|:)", lower_line):
        return "Changed"
    if re.search(r"(^|\W)test(\(|:)", lower_line):
        return "Other"
    if re.search(r"(^|\W)build(\(|:)", lower_line):
        return "Other"
    if re.search(r"(^|\W)ci(\(|:)", lower_line):
        return "Other"
    if re.search(r"(^|\W)chore(\(|:)", lower_line):
        return "Other"
    if re.search(r"(^|\W)revert(\(|:)", lower_line):
        return "Changed"

    # breaking changes:
    # - "BREAKING CHANGE(S)" anywhere
    # - conventional-commit "!" marker before ":"
    if "breaking change" in lower_line or "breaking changes" in lower_line:
        return "Changed"
    prefix = lower_line.split(":", 1)[0]
    if "!" in prefix:
        return "Changed"

    # keyword heuristics
    if "deprecat" in lower_line:
        return "Deprecated"
    if "remove" in lower_line or "deleted" in lower_line or "drop " in lower_line:
        return "Removed"

    return "Other"


def _extract_items(body: str) -> List[str]:
    """Extract Conventional-Commit-like entries from the release body.

    Accepted inputs:
    - Bullet/numbered list lines ("* ...", "- ...", "1. ...") that, after cleaning,
      start with a Conventional Commit prefix (e.g. "feat:", "fix(ui):").
    - Non-bullet lines that already start with a Conventional Commit prefix.

    Everything else is excluded (e.g. "Full Changelog" links, prose paragraphs).

    Filtering:
    - Omit Dependabot dependency bump entries like:
      "build(deps): ... by @dependabot[bot] in https://.../pull/123"
    """
    lines = body.splitlines()

    candidates: List[str] = []
    for raw in lines:
        if not raw.strip():
            continue
        if _is_heading(raw):
            continue

        cleaned = ""
        if re.match(r"^\s*[-*]\s+\S", raw) or re.match(r"^\s*\d+\.\s+\S", raw):
            cleaned = _clean_line(raw)
        else:
            cleaned = raw.strip()

        # Only accept conventional-commit-like entries.
        if not _looks_like_conventional_commit(cleaned):
            continue

        candidates.append(cleaned)

    # Apply filtering + de-duplicate while preserving order
    seen = set()
    out: List[str] = []
    for item in candidates:
        parsed = _parse_release_note_item(item)
        if parsed and _is_dependabot_deps_entry(parsed):
            continue

        if item in seen:
            continue
        seen.add(item)
        out.append(item)

    return out




def build_payload(
    *,
    tag: str,
    version: str,
    date: str,
    body: str,
    source_repo: str,
) -> Dict:
    version = _derive_version(tag=tag, version=version)
    payload = _init_payload(
        version=version, tag=tag, date=date, source_repo=source_repo
    )

    for item in _extract_items(body):
        section = _bucket_for(item)
        payload["keep_a_changelog"][section].append(item)

    return payload


def _parse_args(argv: List[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Generate Keep a Changelog JSON from GitHub Release metadata.",
    )
    p.add_argument("--tag", default="", help='Release tag, e.g. "v1.2.3"')
    p.add_argument(
        "--version", default="", help='Release version, e.g. "1.2.3" (optional)'
    )
    p.add_argument("--date", default="", help="Release date/time (RFC3339 preferred)")
    p.add_argument("--body", default="", help="Release notes body (markdown/text)")
    p.add_argument("--source-repo", default="", help='Source repo as "owner/repo"')
    p.add_argument(
        "--pretty",
        action="store_true",
        help="Pretty-print JSON (indent=2). Default is compact.",
    )
    return p.parse_args(argv)


def main(argv: List[str]) -> int:
    args = _parse_args(argv)

    tag = (
        args.tag
        or _env("RELEASE_TAG")
        or _env("GITHUB_REF_NAME")
        or _env("RELEASE_VERSION")
    )
    version = args.version or _env("RELEASE_VERSION")
    date = (
        args.date
        or _env("RELEASE_DATE")
        or _env("RELEASE_PUBLISHED_AT")
        or _env("GITHUB_EVENT_RELEASE_PUBLISHED_AT")
    )
    body = (
        args.body
        or os.environ.get("RELEASE_BODY")
        or os.environ.get("GITHUB_EVENT_RELEASE_BODY")
        or ""
    )
    source_repo = args.source_repo or _env("SOURCE_REPO") or _env("GITHUB_REPOSITORY")

    payload = build_payload(
        tag=tag,
        version=version,
        date=date,
        body=body,
        source_repo=source_repo,
    )

    if args.pretty:
        sys.stdout.write(json.dumps(payload, ensure_ascii=False, indent=2))
        sys.stdout.write("\n")
    else:
        sys.stdout.write(json.dumps(payload, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
