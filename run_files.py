#!/usr/bin/env python3
import argparse
import os
import subprocess
import tempfile
from enum import Enum
from pathlib import Path

EXEC_PATH = ["./target/debug/sourcery-analyzer", "file"]

gofiles = [
    [
        "./toanalyze/aoc/golang/advent-of-code-my-solutions/go/2024/days/d01/d01.go",
        "./toanalyze/aoc/golang/advent-of-code-my-solutions/go/2024/days/d01/utils.go",
    ],
    [
        "./toanalyze/aoc/golang/advent-of-code-my-solutions/go/2024/days/d02/d02.go",
        "./toanalyze/aoc/golang/advent-of-code-my-solutions/go/2024/days/d02/utils.go",
    ],
    [
        "./toanalyze/aoc/golang/advent-of-code-my-solutions/go/2024/days/d03/d03.go",
        "./toanalyze/aoc/golang/advent-of-code-my-solutions/go/2024/days/d03/utils.go",
    ],
]

ocamlfiles = [
    "./toanalyze/aoc/ocaml/advent-of-code/2024/Day01/puzzle.ml",
    "./toanalyze/aoc/ocaml/advent-of-code/2024/Day02/puzzle.ml",
    "./toanalyze/aoc/ocaml/advent-of-code/2024/Day03/puzzle.ml",
]


class FileType(Enum):
    GOLANG = ("go", "golang")
    OCAML = ("ml", "ocaml")

    @property
    def extension(self):
        return self.value[0]

    @property
    def analyzer_arg(self):
        return self.value[1]


def build():
    cmd = ["cargo", "build"]
    subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def combine(files, filetype):
    fp = tempfile.NamedTemporaryFile(
        mode="w", suffix=f".{filetype.extension}", delete=False
    )
    res = ""
    for file in files:
        with open(file, "r") as f:
            res += f.read()
    fp.write(res)
    fp.close()
    return fp.name


def outfile_for(file):
    path = Path(file)
    return str(path.with_name(f"{path.stem}.stats.txt"))


def analyze_file(file, filetype):
    infile = file
    if isinstance(file, list):
        infile = combine(file, filetype)

    outfile = outfile_for(file[0] if isinstance(file, list) else file)
    res = subprocess.run(
        [*EXEC_PATH, infile, "--outfile", outfile, filetype.analyzer_arg]
    )

    if isinstance(file, list):
        os.unlink(infile)

    return res.returncode


def analyze_files(files, filetype):
    for file in files:
        if analyze_file(file, filetype) != 0:
            print(f"Error for file {file}")
        else:
            print(f"Pass file {file}")


def parse_args():
    a = argparse.ArgumentParser()
    a.add_argument("--download", action=argparse.BooleanOptionalAction)
    a.add_argument("--build", action=argparse.BooleanOptionalAction)
    return a.parse_args()


if __name__ == "__main__":
    args = parse_args()
    if args.download:
        prefix = ["gh", "repo", "clone"]
        postfix = ["--", "--depth", "1"]
        downloads = [
            [
                *prefix,
                "dlesbre/advent-of-code",
                "./toanalyze/aoc/ocaml/advent-of-code",
                *postfix,
            ],
            [
                *prefix,
                "ColasNahaboo/advent-of-code-my-solutions",
                "./toanalyze/aoc/ocaml/advent-of-code-my-solutions",
                *postfix,
            ],
        ]
        for download in downloads:
            res = subprocess.run(download)
            if res.returncode != 0:
                print(f"WARN: could not download repo {download}")

    if args.build:
        print("Building...")
        build()

    analyze_files(gofiles, FileType.GOLANG)
    analyze_files(ocamlfiles, FileType.OCAML)
