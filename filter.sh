#!/usr/bin/env bash

FOLDER_ID="mfr7r-ktntf"

while IFS= read -r path || [[ -n "$path" ]]; do
    [[ -z "$path" || "$path" == \#* ]] && continue

    query_path=${path#./}
    result=$(
        syncthing cli debug file "$FOLDER_ID" "$query_path" 2>/dev/null |
        jq -r '.local | if .name == "" then "NOT-INDEXED" elif .ignored then "IGNORED" else "SYNCED" end'
    )

    printf '%-8s %s\n' "$result" "$path"
done < dirs.txt
