#!/usr/bin/env bash

echo "Get codebases find first go codebase"
id=$(curl localhost:8000/codebases 2>/dev/null | jq .Golang[0].id | tr -d '"')
echo "Found ID! ID is $id"
echo "Getting codebases"
echo "curl -X GET http://localhost:8000/codebases/$id"

codebases=$(curl -X GET "http://localhost:8000/codebases/$id" 2>/dev/null)
echo "$codebases" | jq


versions=$(curl -X GET "http://localhost:8000/codebases/$id/metrics" 2>/dev/null | jq length)
echo "Found $versions Versions"
