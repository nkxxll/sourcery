#!/usr/bin/env python3
import argparse
import csv
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any


TIMELINE_FIELDS = [
    "event",
    "created_at",
    "actor",
    "commit_id",
    "label",
    "assignee",
    "milestone",
    "source",
    "body",
    "html_url",
]


def gh_api(path: str, params: dict[str, str] | None = None) -> Any:
    cmd = ["gh", "api", "--method", "GET", "--paginate", path]
    for key, value in (params or {}).items():
        if value != "":
            cmd.extend(["-f", f"{key}={value}"])

    try:
        res = subprocess.run(cmd, check=True, text=True, capture_output=True)
    except FileNotFoundError:
        sys.exit("error: gh is not installed or is not on PATH")
    except subprocess.CalledProcessError as exc:
        message = exc.stderr.strip() or exc.stdout.strip() or str(exc)
        sys.exit(f"error: gh api failed for {path}: {message}")

    decoder = json.JSONDecoder()
    data = []
    rest = res.stdout.strip()
    while rest:
        value, idx = decoder.raw_decode(rest)
        data.extend(value if isinstance(value, list) else [value])
        rest = rest[idx:].strip()
    return data


def parse_time(value: str | None) -> datetime | None:
    if not value:
        return None
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError:
        sys.exit(f"error: invalid timestamp: {value}")
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed


def in_range(value: str | None, since: datetime | None, until: datetime | None) -> bool:
    parsed = parse_time(value)
    if parsed is None:
        return False
    if since and parsed < since:
        return False
    if until and parsed > until:
        return False
    return True


def user_login(value: dict[str, Any] | None) -> str:
    return value.get("login", "") if value else ""


def nested_name(value: dict[str, Any] | None) -> str:
    return value.get("name", "") if value else ""


def timeline_body(item: dict[str, Any]) -> str:
    body = item.get("body") or ""
    return " ".join(body.split())


def normalize_timeline_item(issue: dict[str, Any], item: dict[str, Any]) -> dict[str, str]:
    event = item.get("event") or "commented"
    return {
        "issue": str(issue["number"]),
        "issue_title": issue.get("title", ""),
        "issue_state": issue.get("state", ""),
        "event": event,
        "created_at": item.get("created_at", ""),
        "actor": user_login(item.get("actor") or item.get("user")),
        "commit_id": item.get("commit_id") or "",
        "label": nested_name(item.get("label")),
        "assignee": user_login(item.get("assignee")),
        "milestone": nested_name(item.get("milestone")),
        "source": source_url(item.get("source")),
        "body": timeline_body(item),
        "html_url": item.get("html_url") or issue.get("html_url", ""),
    }


def source_url(source: dict[str, Any] | None) -> str:
    if not source:
        return ""
    issue = source.get("issue") or {}
    return issue.get("html_url", "")


def issue_opened_item(issue: dict[str, Any]) -> dict[str, str]:
    return {
        "issue": str(issue["number"]),
        "issue_title": issue.get("title", ""),
        "issue_state": issue.get("state", ""),
        "event": "opened",
        "created_at": issue.get("created_at", ""),
        "actor": user_login(issue.get("user")),
        "commit_id": "",
        "label": "",
        "assignee": "",
        "milestone": nested_name(issue.get("milestone")),
        "source": "",
        "body": timeline_body(issue),
        "html_url": issue.get("html_url", ""),
    }


def fetch_issues(args: argparse.Namespace) -> list[dict[str, Any]]:
    params = {
        "state": args.state,
        "sort": args.sort,
        "direction": args.direction,
        "per_page": "100",
    }
    if args.label:
        params["labels"] = ",".join(args.label)
    if args.assignee:
        params["assignee"] = args.assignee
    if args.author:
        params["creator"] = args.author
    if args.mentioned:
        params["mentioned"] = args.mentioned
    if args.milestone:
        params["milestone"] = args.milestone
    if args.updated_since:
        params["since"] = args.updated_since

    issues = gh_api(f"repos/{args.repo}/issues", params)
    issues = [issue for issue in issues if "pull_request" not in issue]
    if args.issue:
        selected = {str(number) for number in args.issue}
        issues = [issue for issue in issues if str(issue["number"]) in selected]
    return issues[: args.limit] if args.limit else issues


def build_timeline(args: argparse.Namespace) -> list[dict[str, str]]:
    since = parse_time(args.since)
    until = parse_time(args.until)
    rows = []

    for issue in fetch_issues(args):
        if (not args.event or "opened" in args.event) and in_range(issue.get("created_at"), since, until):
            rows.append(issue_opened_item(issue))

        endpoint = f"repos/{args.repo}/issues/{issue['number']}/timeline"
        for item in gh_api(endpoint, {"per_page": "100"}):
            event = item.get("event") or "commented"
            if args.event and event not in args.event:
                continue
            if in_range(item.get("created_at"), since, until):
                rows.append(normalize_timeline_item(issue, item))

    return sorted(rows, key=lambda row: (row["created_at"], int(row["issue"])))


def write_json(rows: list[dict[str, str]]) -> None:
    json.dump(rows, sys.stdout, indent=2)
    print()


def write_csv(rows: list[dict[str, str]]) -> None:
    writer = csv.DictWriter(sys.stdout, fieldnames=["issue", "issue_title", "issue_state", *TIMELINE_FIELDS])
    writer.writeheader()
    writer.writerows(rows)


def write_markdown(rows: list[dict[str, str]]) -> None:
    print("| Time | Issue | Event | Actor | Details |")
    print("| --- | --- | --- | --- | --- |")
    for row in rows:
        details = detail_text(row).replace("|", "\\|")
        issue = f"[#{row['issue']}]({row['html_url']}) {row['issue_title']}".replace("|", "\\|")
        print(f"| {row['created_at']} | {issue} | {row['event']} | {row['actor']} | {details} |")


def detail_text(row: dict[str, str]) -> str:
    parts = []
    for key in ["label", "assignee", "milestone", "commit_id", "source"]:
        if row[key]:
            parts.append(f"{key}={row[key]}")
    if row["body"]:
        parts.append(row["body"][:200])
    return "; ".join(parts)


def save_to_postgres(database_url: str, repo: str, rows: list[dict[str, str]]) -> None:
    rows = dedupe_rows(rows)
    payload = json.dumps(rows)
    tag = "sourcery_issue_timeline_json"
    sql = f"""
WITH incoming AS (
    SELECT *
    FROM jsonb_to_recordset(${tag}${payload}${tag}$::jsonb) AS row_data(
        issue text,
        issue_title text,
        issue_state text,
        event text,
        created_at text,
        actor text,
        commit_id text,
        label text,
        assignee text,
        milestone text,
        source text,
        body text,
        html_url text
    )
)
INSERT INTO github_issue_timeline_events (
    repo,
    issue_number,
    issue_title,
    issue_state,
    event,
    event_created_at,
    actor,
    commit_id,
    label,
    assignee,
    milestone,
    source,
    body,
    html_url,
    raw_event
)
SELECT
    {sql_literal(repo)},
    issue::integer,
    COALESCE(issue_title, ''),
    COALESCE(issue_state, ''),
    COALESCE(event, ''),
    created_at::timestamptz,
    COALESCE(actor, ''),
    COALESCE(commit_id, ''),
    COALESCE(label, ''),
    COALESCE(assignee, ''),
    COALESCE(milestone, ''),
    COALESCE(source, ''),
    COALESCE(body, ''),
    COALESCE(html_url, ''),
    to_jsonb(incoming)
FROM incoming
WHERE issue IS NOT NULL AND created_at IS NOT NULL AND event IS NOT NULL
ON CONFLICT (repo, issue_number, event, event_created_at, actor, html_url)
DO UPDATE SET
    issue_title = EXCLUDED.issue_title,
    issue_state = EXCLUDED.issue_state,
    commit_id = EXCLUDED.commit_id,
    label = EXCLUDED.label,
    assignee = EXCLUDED.assignee,
    milestone = EXCLUDED.milestone,
    source = EXCLUDED.source,
    body = EXCLUDED.body,
    raw_event = EXCLUDED.raw_event,
    imported_at = now();
"""
    try:
        subprocess.run(["psql", database_url, "--quiet", "--set", "ON_ERROR_STOP=1"], input=sql, text=True, check=True)
    except FileNotFoundError:
        sys.exit("error: psql is not installed or is not on PATH")
    except subprocess.CalledProcessError as exc:
        sys.exit(f"error: failed to save timeline rows to postgres: {exc}")


def sql_literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def dedupe_rows(rows: list[dict[str, str]]) -> list[dict[str, str]]:
    deduped = {}
    for row in rows:
        key = (
            row["issue"],
            row["event"],
            row["created_at"],
            row["actor"],
            row["html_url"],
        )
        deduped[key] = row
    return list(deduped.values())


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a chronological GitHub issue timeline for a repository. Requires gh auth login."
    )
    parser.add_argument("repo", help="Repository in OWNER/REPO form")
    parser.add_argument("--state", choices=["open", "closed", "all"], default="all")
    parser.add_argument("--label", action="append", help="Require label; repeat for multiple labels")
    parser.add_argument("--assignee", help="Filter issues by assignee login, 'none', or '*' for any")
    parser.add_argument("--author", help="Filter issues by creator login")
    parser.add_argument("--mentioned", help="Filter issues mentioning this login")
    parser.add_argument("--milestone", help="Filter by milestone number, 'none', or '*' for any")
    parser.add_argument("--issue", type=int, action="append", help="Only include an issue number; repeatable")
    parser.add_argument("--event", action="append", help="Only include this timeline event type; repeatable")
    parser.add_argument("--since", help="Only include timeline entries at or after this ISO timestamp/date")
    parser.add_argument("--until", help="Only include timeline entries at or before this ISO timestamp/date")
    parser.add_argument("--updated-since", help="Only fetch issues updated at or after this ISO timestamp")
    parser.add_argument("--limit", type=int, help="Maximum number of matching issues to fetch")
    parser.add_argument("--sort", choices=["created", "updated", "comments"], default="created")
    parser.add_argument("--direction", choices=["asc", "desc"], default="asc")
    parser.add_argument("--format", choices=["markdown", "json", "csv"], default="markdown")
    parser.add_argument(
        "--database-url",
        default=os.environ.get("DATABASE_URL"),
        help="Postgres connection string. Defaults to DATABASE_URL.",
    )
    parser.add_argument("--save", action="store_true", help="Upsert timeline rows into Postgres")
    parser.add_argument("--no-output", action="store_true", help="Do not print the timeline")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    rows = build_timeline(args)
    if args.save:
        if not args.database_url:
            sys.exit("error: --save requires --database-url or DATABASE_URL")
        save_to_postgres(args.database_url, args.repo, rows)
    if args.no_output:
        return
    if args.format == "json":
        write_json(rows)
    elif args.format == "csv":
        write_csv(rows)
    else:
        write_markdown(rows)


if __name__ == "__main__":
    main()
