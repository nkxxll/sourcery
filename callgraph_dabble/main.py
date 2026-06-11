# functions
# X calls:
#   Y
#
# Y calls:
#   Z
#   Z1
#
# A calls:
#   B
#   C
#   D
#
# B calls:
#   Nothing
#
# C calls:
#   Nothing
#
# D calls:
#   F (args: X)
#
# F calls:
#   Nothing
data = {
    "X": [{"name": "Y", "args": []}],
    "Y": [{"name": "Z", "args": []}, {"name": "Z1", "args": []}],
    "A": [
        {"name": "B", "args": []},
        {"name": "C", "args": []},
        {"name": "D", "args": []},
    ],
    "B": [],
    "C": [],
    "D": [{"name": "F", "args": ["X"]}],
    "F": [],
}

merged = {}
children = {}


for key in data.keys():
    c = data[key]
    children[c["name"]] = c["args"]

for key in data.keys():
    merged[key] = []



def dfs(node, path: set):
    children = data.get(node, [])
    m = -1
    for child in children:
        if child["name"] not in path:
            path.add(child["name"])
            m = max(m, 1 + dfs(child["name"], path))
    return m


print(dfs("A", set()))
