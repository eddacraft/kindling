#!/usr/bin/env python3
"""Check Kindling's two release records and optionally extract release notes."""
import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADING = re.compile(r"^## \\[([^\\]]+)\\](?: - (\\d{4}-\\d{2}-\\d{2}))?\\s*$", re.M)
LINKS = {
    "CHANGELOG.md": "ENGINEERING-HISTORY.md",
    "ENGINEERING-HISTORY.md": "CHANGELOG.md",
}

def sections(text):
    matches = list(HEADING.finditer(text))
    return [(m.group(1), m.group(2), text[m.end():matches[i+1].start() if i+1<len(matches) else len(text)].strip()) for i,m in enumerate(matches)]

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--release-version", help="require a matching version in both records")
    p.add_argument("--notes-version", help="extract the user-facing section for this version")
    p.add_argument("--notes-output", type=pathlib.Path, help="write extracted notes to this file")
    args = p.parse_args()
    errors = []
    records = {}
    for name, other in LINKS.items():
        text = (ROOT/name).read_text(encoding="utf-8")
        entries = sections(text)
        records[name] = entries
        names = [v for v, _, _ in entries]
        if not entries or names[0] != "Unreleased":
            errors.append(f"{name}: first version heading must be [Unreleased]")
        if len(names) != len(set(names)):
            errors.append(f"{name}: duplicate version heading")
        if not re.search(r"\\]\(\\./"+re.escape(other)+r"\\)", text):
            errors.append(f"{name}: missing cross-link to {other}")
        for v,date,_ in entries:
            if v != "Unreleased" and not date:
                errors.append(f"{name}: {v} missing release date")
            if v == "Unreleased" and date:
                errors.append(f"{name}: Unreleased must not have a date")
    a = [(v,d) for v,d,_ in records["CHANGELOG.md"]]
    b = [(v,d) for v,d,_ in records["ENGINEERING-HISTORY.md"]]
    if a != b:
        errors.append("Release versions/dates or ordering differ between changelog and engineering history")
    if args.release_version and args.release_version not in dict(a):
        errors.append(f"Release {args.release_version} not documented in both files")
    if args.notes_version:
        section = next((body for v,_,body in records["CHANGELOG.md"] if v==args.notes_version), None)
        if not section:
            errors.append(f"No user-facing release notes found for {args.notes_version}")
        elif not args.notes_output:
            errors.append("--notes-output required with --notes-version")
        elif not errors:
            args.notes_output.write_text(section+"\\n", encoding="utf-8")
    if errors:
        for e in errors:
            print("ERROR:",e,file=sys.stderr)
        return 1
    print(f"Release docs OK: {len(a)-1} historical versions, matching dates and cross-links")
    return 0

if __name__ == "__main__":
    sys.exit(main())
