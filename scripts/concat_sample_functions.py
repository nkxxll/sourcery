#!/usr/bin/env python3
import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse


SUPPORTED_LANGUAGES = {
    "go": "go_sample_functions2.go",
    "ocaml": "ocaml_sample_functions2.ml",
}

COMMENT_PREFIXES = {
    "go": ("// ", ""),
    "ocaml": ("(* ", " *)"),
}

SAMPLE_HEADING_RE = re.compile(r"^===\s+(?P<language>Go|OCaml)\s*$", re.IGNORECASE)
GITHUB_BLOB_RE = re.compile(
    r"^/(?P<owner>[^/]+)/(?P<repo>[^/]+)/blob/(?P<commit>[^/]+)/(?P<path>.+)$"
)
LINE_RANGE_RE = re.compile(r"^L(?P<start>\d+)(?:-L(?P<end>\d+))?$")


@dataclass(frozen=True)
class Sample:
    language: str
    url: str
    owner: str
    repo: str
    commit: str
    source_path: str
    start_line: int
    end_line: int


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def parse_sample_url(language: str, url: str) -> Sample:
    parsed = urlparse(url)
    if parsed.netloc != "github.com":
        raise ValueError(f"not a GitHub URL: {url}")

    blob_match = GITHUB_BLOB_RE.match(parsed.path)
    if blob_match is None:
        raise ValueError(f"not a GitHub blob URL: {url}")

    range_match = LINE_RANGE_RE.match(parsed.fragment)
    if range_match is None:
        raise ValueError(f"missing or invalid line range in URL: {url}")

    start_line = int(range_match.group("start"))
    end_line = int(range_match.group("end") or start_line)
    if end_line < start_line:
        raise ValueError(f"line range ends before it starts: {url}")

    return Sample(
        language=language,
        url=url,
        owner=blob_match.group("owner"),
        repo=blob_match.group("repo"),
        commit=blob_match.group("commit"),
        source_path=blob_match.group("path"),
        start_line=start_line,
        end_line=end_line,
    )


def parse_samples(path: Path) -> list[Sample]:
    samples: list[Sample] = []
    current_language: str | None = None

    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw_line.strip()
        heading_match = SAMPLE_HEADING_RE.match(line)
        if heading_match is not None:
            current_language = heading_match.group("language").lower()
            continue

        if "https://github.com/" not in line:
            continue
        if current_language not in SUPPORTED_LANGUAGES:
            raise ValueError(f"sample URL before supported language heading on line {line_number}: {line}")

        url = line[line.index("https://github.com/") :].strip()
        samples.append(parse_sample_url(current_language, url))

    return samples


def checked_out_repo(sample: Sample, listed_dir: Path) -> Path:
    return listed_dir / sample.language / f"{sample.owner}__{sample.repo}"


def git_show(repo_dir: Path, sample: Sample) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "show", f"{sample.commit}:{sample.source_path}"],
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"failed to read {sample.commit}:{sample.source_path} from {repo_dir}:\n"
            f"{result.stderr.strip()}"
        )
    return result.stdout.splitlines()


def extract_sample(sample: Sample, listed_dir: Path) -> str:
    repo_dir = checked_out_repo(sample, listed_dir)
    if not repo_dir.is_dir():
        raise FileNotFoundError(f"missing repository for {sample.url}: {repo_dir}")

    lines = git_show(repo_dir, sample)
    if sample.end_line > len(lines):
        raise ValueError(
            f"line range {sample.start_line}-{sample.end_line} exceeds {sample.source_path} "
            f"length {len(lines)} for {sample.url}"
        )

    function_code = "\n".join(lines[sample.start_line - 1 : sample.end_line])
    comment_start, comment_end = COMMENT_PREFIXES[sample.language]
    return (
        f"{comment_start}{sample.url}{comment_end}\n"
        f"{comment_start}{sample.owner}/{sample.repo} "
        f"{sample.source_path}:{sample.start_line}-{sample.end_line}{comment_end}\n"
        f"{function_code}\n"
    )


def write_outputs(samples: list[Sample], listed_dir: Path, out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    by_language = {language: [] for language in SUPPORTED_LANGUAGES}
    for sample in samples:
        by_language[sample.language].append(extract_sample(sample, listed_dir))

    for language, output_name in SUPPORTED_LANGUAGES.items():
        output_path = out_dir / output_name
        output_path.write_text("\n".join(by_language[language]), encoding="utf-8")
        print(f"wrote {len(by_language[language])} {language} samples to {output_path}")


def parse_args() -> argparse.Namespace:
    root = repo_root()
    parser = argparse.ArgumentParser(
        description="Concatenate sampled Go and OCaml function snippets from samples.txt."
    )
    parser.add_argument("--samples", type=Path, default=root / "samples2.txt")
    parser.add_argument("--listed-dir", type=Path, default=root / "toanalyze" / "listed")
    parser.add_argument("--out-dir", type=Path, default=root)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        samples = parse_samples(args.samples)
        write_outputs(samples, args.listed_dir, args.out_dir)
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
