#!/usr/bin/env python3
import subprocess
from datetime import datetime

LIMIT = 30
LANGUAGES = ["go", "ocaml"]
FILTER = ["stars", "forks"]
# createdAt
# defaultBranch
# description
# forksCount
# fullName
# hasDownloads
# hasIssues
# hasPages
# hasProjects
# hasWiki
# homepage
# id
# isArchived
# isDisabled
# isFork
# isPrivate
# language
# license
# name
# openIssuesCount
# owner
# pushedAt
# size
# stargazersCount
# updatedAt
# url
# visibility
# watchersCount
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


def file_name(lang, filter, limit):
    now = datetime.now()
    now_string = now.isoformat()
    return f"{lang}_{filter}_{limit}_{now_string}".replace(":", "_").replace(".", "_")


def build_command(lang, filter, limit):
    return [
        "gh",
        "search",
        "repos",
        f"--language={lang}",
        f"--sort={filter}",
        f"--limit={LIMIT}",
        f"--json={','.join(JSON_FIELDS)}",
    ]


for lang in LANGUAGES:
    for fil in FILTER:
        with open(file_name(lang, fil, LIMIT), mode="w") as f:
            subprocess.run(build_command(lang, fil, LIMIT), stdout=f)
