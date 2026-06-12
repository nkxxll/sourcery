#!/usr/bin/env python3
import argparse
import json
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Any


JSON_FIELDS = [
    "id",
    "createdAt",
    "name",
    "fullName",
    "description",
    "owner",
    "language",
    "size",
    "url",
    "watchersCount",
    "updatedAt",
    "forksCount",
    "stargazersCount",
]

LANGUAGE_ARGS = {
    "go": "golang",
    "ocaml": "ocaml",
}


def run(cmd: list[str], cwd: Path | None = None, capture: bool = False) -> subprocess.CompletedProcess[str]:
    print("+ " + " ".join(cmd), flush=True)
    return subprocess.run(
        cmd,
        cwd=cwd,
        check=True,
        text=True,
        capture_output=capture,
    )


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def query_file_name(language: str, limit: int) -> str:
    timestamp = datetime.now().isoformat(timespec="microseconds")
    safe_timestamp = timestamp.replace(":", "_").replace(".", "_")
    return f"{language}_stars_{limit}_{safe_timestamp}"


def query_top_starred(language: str, limit: int, query_dir: Path) -> tuple[Path, list[dict[str, Any]]]:
    query_dir.mkdir(parents=True, exist_ok=True)
    cmd = [
        "gh",
        "search",
        "repos",
        f"--language={language}",
        "--sort=stars",
        f"--limit={limit}",
        f"--json={','.join(JSON_FIELDS)}",
    ]
    result = run(cmd, capture=True)
    repos = json.loads(result.stdout)

    out_path = query_dir / query_file_name(language, limit)
    out_path.write_text(json.dumps(repos, indent=2) + "\n", encoding="utf-8")
    print(f"wrote query results to {out_path}")
    return out_path, repos


def load_repos(path: Path) -> list[dict[str, Any]]:
    return json.loads(path.read_text(encoding="utf-8"))


def clone_or_update(repo: dict[str, Any], language: str, clone_root: Path) -> Path:
    full_name = repo["fullName"]
    destination = clone_root / language / full_name.replace("/", "__")
    destination.parent.mkdir(parents=True, exist_ok=True)

    if destination.exists():
        run(["git", "-C", str(destination), "fetch", "--all", "--tags", "--prune"])
    else:
        run(["git", "clone", f"https://github.com/{full_name}.git", str(destination)])

    return destination


def analyze(path: Path, samples: int, language: str, root: Path) -> None:
    run(
        [
            "cargo",
            "run",
            "-p",
            "sourcery-analyzer",
            "--",
            "sample",
            str(path),
            str(samples),
            LANGUAGE_ARGS[language],
        ],
        cwd=root,
    )


def parse_args() -> argparse.Namespace:
    root = repo_root()
    parser = argparse.ArgumentParser(
        description="Download the top starred Go and OCaml repos and run sampled analysis."
    )
    parser.add_argument("--limit", type=int, default=10, help="repos per language to query")
    parser.add_argument("--samples", type=int, default=10, help="samples per repo analysis")
    parser.add_argument(
        "--languages",
        nargs="+",
        choices=sorted(LANGUAGE_ARGS),
        default=["go", "ocaml"],
        help="languages to query and analyze",
    )
    parser.add_argument(
        "--query-dir",
        type=Path,
        default=root / "gh_query",
        help="directory for GitHub query JSON output",
    )
    parser.add_argument(
        "--clone-dir",
        type=Path,
        default=root / "toanalyze" / "top_starred",
        help="directory for cloned repositories",
    )
    parser.add_argument(
        "--from-query",
        type=Path,
        action="append",
        default=[],
        help="use an existing gh_query JSON file instead of querying GitHub; repeat per language",
    )
    parser.add_argument("--no-analyze", action="store_true", help="clone/update only")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = repo_root()
    query_inputs = list(args.from_query)
    failures = []

    for index, language in enumerate(args.languages):
        if query_inputs:
            if index >= len(query_inputs):
                print(f"error: missing --from-query file for language {language}", file=sys.stderr)
                return 2
            query_path = query_inputs[index]
            repos = load_repos(query_path)[: args.limit]
            print(f"loaded {len(repos)} {language} repos from {query_path}")
        else:
            _, repos = query_top_starred(language, args.limit, args.query_dir)

        for repo in repos:
            full_name = repo.get("fullName", "<unknown>")
            try:
                repo_path = clone_or_update(repo, language, args.clone_dir)
                if not args.no_analyze:
                    analyze(repo_path, args.samples, language, root)
            except (KeyError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
                failures.append((language, full_name, str(exc)))
                print(f"error: failed {language} repo {full_name}: {exc}", file=sys.stderr)

    if failures:
        print("\nFailures:", file=sys.stderr)
        for language, full_name, message in failures:
            print(f"- {language} {full_name}: {message}", file=sys.stderr)
        return 1

    print("completed top-starred repo download and sampled analysis")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
