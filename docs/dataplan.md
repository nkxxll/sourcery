# Data Model for Code Metrics

**Date:** 2026-03-26
**Source:** [chatgpt.com](https://chatgpt.com/share/69c54339-91d0-8329-81dc-9029a77e48b4)

---

### **You**

lets jab about a data model I want to write an application that queries code for specific metrics on different levels of detail. there will also be a lot of different versions of the same base codebase.

codebase:
version:
directories: (for if I want to have the metrics of one "module" or directory i'll need to query by directory name like "src/utils" which will be unique)
file:
function:

and all of these levels will have different levels that depending on the metric will or will not be computable by the lower levels e.g. file loc cannot be computed by function loc because there might be lines between functions that are unique to the file in this case. how would I build the data model which database would you recommend and why what are my options and how would you compute those metrics

---

### **ChatGPT**

This is a great problem space—you’re basically designing a **code analytics system with hierarchical aggregation and versioning**, which has some non-obvious tradeoffs. Let’s break it down cleanly so you don’t paint yourself into a corner.

---

# 1. Core challenge (what actually matters)

You have:

- A **hierarchy**  
  `codebase → version → directory → file → function`

- **Metrics at multiple levels**, where:
  - Some **can be aggregated upward** (e.g. function LOC → file LOC *partially*)
  - Some **cannot be derived bottom-up** (e.g. file LOC ≠ sum of function LOC)

- **Multiple versions of the same codebase** (diffing, history, trends)

👉 The key design decision:
> Do you store *only raw data and recompute*, or *store precomputed metrics per level*?

Answer: **you will need both**.

---

# 2. Recommended database choice

### ✅ Best default: **relational DB (PostgreSQL)**

Why:
- Strong support for **hierarchies + joins**
- Can enforce **uniqueness (e.g. directory path per version)**
- Handles **aggregation queries well**
- Supports **JSONB** for flexible metrics
- Mature indexing (critical for performance)

---

### When NOT to use Postgres

| Option | When to use | Why not default |
|------|------------|----------------|
| Document DB (MongoDB) | If structure is highly variable | Hard to query across hierarchy efficiently |
| Graph DB (Neo4j) | If relationships are complex (calls, dependencies) | Overkill for hierarchical metrics |
| Column DB (ClickHouse) | If you mainly do analytics at scale | Weak for transactional ingestion |
| Time-series DB | If tracking metrics over time only | Doesn’t model hierarchy well |

👉 If you later want **fast analytics dashboards**, you can replicate into something like ClickHouse.

---

# 3. Data model (clean and scalable)

## Core entities

### Codebase
```
codebases
- id
- name
```

### Version
```
versions
- id
- codebase_id
- commit_hash / version_tag
- created_at
```

---

### Directory
```
directories
- id
- version_id
- path  (e.g. "src/utils")
- parent_id (self reference)
```

👉 Important: store full path **and** parent tree

---

### File
```
files
- id
- version_id
- directory_id
- path
- language
```

---

### Function
```
functions
- id
- file_id
- name
- start_line
- end_line
```

---

# 4. Metrics design (THIS is the tricky part)

You should NOT try to infer everything dynamically.

Instead:

## ✅ Store metrics per entity level

```
metrics
- id
- entity_type  (codebase | version | directory | file | function)
- entity_id
- metric_name  (loc, complexity, etc.)
- metric_value
```

---

### Alternative (better): JSONB per entity

```
file_metrics
- file_id
- metrics JSONB
```

Example:
```json
{
  "loc": 120,
  "comment_loc": 30,
  "complexity": 15
}
```

👉 This is more flexible and faster to query in practice.

---

# 5. Key design rule (VERY important)

> ❗ Never assume metrics can always be aggregated.

Instead:

### Split metrics into two types:

## 1. Aggregatable metrics
- function LOC
- cyclomatic complexity
- number of functions

👉 Can roll up:
```
file = sum(functions) + adjustments
```

---

## 2. Non-aggregatable metrics
- file LOC (true LOC)
- blank lines
- comments outside functions
- imports, headers, etc.

👉 Must be computed **directly at that level**

---

# 6. How to compute metrics (pipeline design)

You want a **pipeline like this**:

```
Code → Parser → AST → Metric Extractor → Store per level
```

---

## Step 1: Parse code

Use language-specific parsers:
- Python → `ast`
- JS → Babel / Esprima
- Multi-language → Tree-sitter (BEST OPTION)

👉 I strongly recommend:
> ⭐ **Tree-sitter**

Why:
- Multi-language
- Incremental parsing
- Gives you precise function boundaries

---

## Step 2: Compute metrics per level

### Function-level
- LOC = end_line - start_line + 1
- complexity (AST traversal)

---

### File-level
- Total LOC (raw file)
- comment LOC
- blank LOC

👉 DO NOT derive from functions

---

### Directory-level
- aggregate:
  - sum(file LOC)
  - sum(complexity)
- OR compute special metrics if needed

---

### Version-level
- aggregate everything
- snapshot of entire repo

---

# 7. Versioning strategy (important)

You have two main options:

---

## Option A: Full snapshot (simpler, recommended first)

Each version stores:
- full directory tree
- full metrics

✅ Pros:
- Easy queries
- No reconstruction logic

❌ Cons:
- Storage heavy

---

## Option B: Delta-based (advanced)

Store only changes between versions

✅ Pros:
- Efficient storage

❌ Cons:
- Query complexity explodes

---

👉 Recommendation:
> Start with **full snapshots**, optimize later if needed

---

# 8. Query examples (what you’ll want)

### Example 1: "LOC of src/utils in version X"
```sql
SELECT SUM(metrics->>'loc')::int
FROM files
JOIN directories ON files.directory_id = directories.id
WHERE directories.path = 'src/utils'
AND files.version_id = ?
```

---

### Example 2: "Trend of complexity over versions"
```sql
SELECT version_id, SUM(metrics->>'complexity')
FROM file_metrics
GROUP BY version_id
```

---

### Example 3: "Worst functions by complexity"
```sql
SELECT *
FROM functions
ORDER BY metrics->>'complexity' DESC
LIMIT 10
```

---

# 9. Performance tips (you’ll need these)

- Index:
  - `version_id`
  - `directory.path`
  - `file.path`
- Use **materialized views** for heavy aggregations
- Cache frequently requested metrics
- Consider **precomputing directory-level metrics**

---

# 10. Clean architecture summary

### Storage
- PostgreSQL
- JSONB metrics

### Processing
- Tree-sitter parsing
- Compute metrics per level independently

### Design principles
- Don’t over-normalize metrics
- Don’t rely on aggregation for correctness
- Store ground truth per level

---

# 11. If you want to go more advanced

You could add:

- **Dependency graph** → then a graph DB makes sense
- **Incremental analysis** → only reprocess changed files
- **Code ownership metrics**
- **Hotspot detection**

---

# 12. One key question for you

Before going further, this will heavily influence the design:

👉 Are you optimizing more for:
1. **Accurate analysis & flexibility** (research-style)
2. **Fast dashboards & queries** (product-style)

Because:
- (1) → more raw data, recomputation
- (2) → more pre-aggregation, denormalization

---

If you want, I can:
- sketch an actual SQL schema
- design the ingestion pipeline
- or show how to model this in an ORM like Prisma / SQLAlchemy

---
