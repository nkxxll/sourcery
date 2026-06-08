CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_file_states_path_trgm
ON file_states USING GIN (path gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_functions_name_trgm
ON functions USING GIN (name gin_trgm_ops);

CREATE OR REPLACE FUNCTION search_version_filenames(
    p_version_id UUID,
    p_query TEXT,
    p_limit INTEGER DEFAULT 50
)
RETURNS TABLE (
    file_state_id UUID,
    file_id UUID,
    path TEXT,
    status TEXT,
    score REAL
)
LANGUAGE SQL
STABLE
AS $$
    WITH target AS (
        SELECT id, created_at, codebase_id
        FROM versions
        WHERE id = p_version_id
    ),
    ranked_file_states AS (
        SELECT
            fs.*,
            row_number() OVER (
                PARTITION BY fs.path
                ORDER BY v.created_at DESC, v.id DESC
            ) AS rank
        FROM file_states fs
        JOIN versions v ON v.id = fs.version_id
        JOIN target t ON TRUE
        WHERE fs.codebase_id = t.codebase_id
          AND (
              v.created_at < t.created_at
              OR (v.created_at = t.created_at AND v.id <= t.id)
          )
    ),
    current_files AS (
        SELECT
            fs.id,
            fs.file_id,
            fs.path,
            fs.status,
            GREATEST(
                similarity(fs.path, p_query),
                similarity(regexp_replace(fs.path, '^.*/', ''), p_query)
            ) AS score
        FROM ranked_file_states fs
        WHERE fs.rank = 1
          AND fs.exists
          AND NULLIF(trim(p_query), '') IS NOT NULL
          AND (
              fs.path % p_query
              OR regexp_replace(fs.path, '^.*/', '') % p_query
              OR fs.path ILIKE '%' || p_query || '%'
          )
    )
    SELECT
        id AS file_state_id,
        file_id,
        path,
        status,
        score
    FROM current_files
    ORDER BY score DESC, path
    LIMIT GREATEST(p_limit, 0);
$$;

CREATE OR REPLACE FUNCTION search_version_functions(
    p_version_id UUID,
    p_query TEXT,
    p_limit INTEGER DEFAULT 50
)
RETURNS TABLE (
    function_id UUID,
    file_id UUID,
    file_path TEXT,
    file_language TEXT,
    name TEXT,
    start_line INTEGER,
    end_line INTEGER,
    score REAL
)
LANGUAGE SQL
STABLE
AS $$
    WITH target AS (
        SELECT id, created_at, codebase_id
        FROM versions
        WHERE id = p_version_id
    ),
    ranked_file_states AS (
        SELECT
            fs.*,
            row_number() OVER (
                PARTITION BY fs.path
                ORDER BY v.created_at DESC, v.id DESC
            ) AS rank
        FROM file_states fs
        JOIN versions v ON v.id = fs.version_id
        JOIN target t ON TRUE
        WHERE fs.codebase_id = t.codebase_id
          AND (
              v.created_at < t.created_at
              OR (v.created_at = t.created_at AND v.id <= t.id)
          )
    ),
    current_functions AS (
        SELECT
            fn.id AS function_id,
            f.id AS file_id,
            fs.path AS file_path,
            f.language AS file_language,
            fn.name,
            fn.start_line,
            fn.end_line,
            GREATEST(
                similarity(fn.name, p_query),
                similarity(fs.path, p_query)
            ) AS score
        FROM ranked_file_states fs
        JOIN files f ON f.id = fs.file_id
        JOIN functions fn ON fn.file_id = f.id
        WHERE fs.rank = 1
          AND fs.exists
          AND NULLIF(trim(p_query), '') IS NOT NULL
          AND (
              fn.name % p_query
              OR fn.name ILIKE '%' || p_query || '%'
              OR fs.path % p_query
              OR fs.path ILIKE '%' || p_query || '%'
          )
    )
    SELECT
        function_id,
        file_id,
        file_path,
        file_language,
        name,
        start_line,
        end_line,
        score
    FROM current_functions
    ORDER BY score DESC, file_path, start_line, name
    LIMIT GREATEST(p_limit, 0);
$$;
