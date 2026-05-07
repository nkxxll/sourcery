ALTER TABLE codebases
ADD COLUMN IF NOT EXISTS programming_language TEXT;

UPDATE codebases
SET programming_language = 'unknown'
WHERE programming_language IS NULL;

ALTER TABLE codebases
ALTER COLUMN programming_language SET NOT NULL;
