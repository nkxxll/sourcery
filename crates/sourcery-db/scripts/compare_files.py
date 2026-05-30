#!/usr/bin/env python3

f1 = open("files.txt")
file1 = f1.readlines()
f1.close()

f2 = open("../../toanalyze/go-yaml/files.txt")
file2 = f2.readlines()
f2.close()

fs1 = set(file1)
fs2 = set(file2)

print(fs1.difference(fs2))
