-- remus: full schema introspection in one query.
--
-- Returns a single text column holding one JSON document. Reads pg_catalog only,
-- never information_schema, never row data: a reader can verify the tool cannot
-- see their data by reading this file. Works standalone, without remus:
--
--     psql "$DATABASE_URL" -Atf introspect.sql > schema.json
--
-- This text is embedded verbatim in the binary and printed by `remus --print-sql`,
-- so the connected path and the paste-the-JSON path can never drift.
-- Requires PostgreSQL 12+.

-- Pin search_path for the duration of this statement (set_config with is_local
-- reverts at end of transaction). Every pg_get_*def / format_type call below then
-- schema-qualifies all names, so the payload does not depend on the caller's
-- search_path and the emitted DDL is unambiguous. `FROM cfg` at the very end is
-- what forces this to run before any of the projection expressions.
WITH cfg AS (
    SELECT set_config('search_path', '', true) AS search_path
),

ns AS (
    SELECT n.oid, n.nspname
    FROM pg_namespace n
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
      AND n.nspname NOT LIKE 'pg\_temp%'
      AND n.nspname NOT LIKE 'pg\_toast\_temp%'
),

rel AS (
    SELECT c.oid,
           n.nspname                                     AS schema,
           c.relname                                     AS name,
           c.relkind::text                               AS kind,
           c.relrowsecurity                              AS rls,
           c.relforcerowsecurity                         AS rls_forced,
           c.relpersistence = 'u'                        AS unlogged,
           obj_description(c.oid, 'pg_class')            AS comment,
           CASE WHEN c.relkind = 'p'
                THEN pg_get_partkeydef(c.oid) END        AS partition_key,
           CASE WHEN c.relkind IN ('v', 'm')
                THEN pg_get_viewdef(c.oid, true) END     AS view_definition
    FROM pg_class c
    JOIN ns n ON n.oid = c.relnamespace
    WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f')
      -- Leaf partitions are one box too many on a diagram; they are listed
      -- separately under `partitions` with their bounds.
      AND NOT c.relispartition
),

cols AS (
    SELECT a.attrelid,
           jsonb_agg(jsonb_build_object(
               'name',        a.attname,
               'type',        format_type(a.atttypid, a.atttypmod),
               -- For arrays, describe the element type: `order_status[]` is
               -- still "an enum column" for every emitter.
               'type_schema', tn.nspname,
               'type_name',   bt.typname,
               'type_kind',   bt.typtype::text,
               'is_array',    et.oid IS NOT NULL,
               'not_null',    a.attnotnull,
               'default',     pg_get_expr(ad.adbin, ad.adrelid),
               'identity',    nullif(a.attidentity, ''),
               'generated',   nullif(a.attgenerated, ''),
               'comment',     col_description(a.attrelid, a.attnum)
           ) ORDER BY a.attnum) AS columns
    FROM pg_attribute a
    JOIN pg_type t        ON t.oid = a.atttypid
    LEFT JOIN pg_type et  ON t.typcategory = 'A' AND et.oid = t.typelem
    JOIN pg_type bt       ON bt.oid = coalesce(et.oid, t.oid)
    JOIN pg_namespace tn  ON tn.oid = bt.typnamespace
    LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum
    WHERE a.attrelid IN (SELECT oid FROM rel)
      AND a.attnum > 0
      AND NOT a.attisdropped
    GROUP BY a.attrelid
),

cons AS (
    SELECT c.conrelid,
           jsonb_agg(jsonb_build_object(
               'name',        c.conname,
               'kind',        c.contype::text,
               'columns',     coalesce((SELECT jsonb_agg(a.attname ORDER BY k.ord)
                                        FROM unnest(c.conkey) WITH ORDINALITY AS k(attnum, ord)
                                        JOIN pg_attribute a
                                          ON a.attrelid = c.conrelid AND a.attnum = k.attnum),
                                       '[]'::jsonb),
               'ref_schema',  rn.nspname,
               'ref_table',   rc.relname,
               'ref_columns', coalesce((SELECT jsonb_agg(a.attname ORDER BY k.ord)
                                        FROM unnest(c.confkey) WITH ORDINALITY AS k(attnum, ord)
                                        JOIN pg_attribute a
                                          ON a.attrelid = c.confrelid AND a.attnum = k.attnum),
                                       '[]'::jsonb),
               'on_update',   CASE WHEN c.contype = 'f' THEN c.confupdtype::text END,
               'on_delete',   CASE WHEN c.contype = 'f' THEN c.confdeltype::text END,
               'deferrable',  c.condeferrable,
               'definition',  pg_get_constraintdef(c.oid),
               'comment',     obj_description(c.oid, 'pg_constraint')
           ) ORDER BY c.contype, c.conname) AS constraints
    FROM pg_constraint c
    LEFT JOIN pg_class rc     ON rc.oid = c.confrelid
    LEFT JOIN pg_namespace rn ON rn.oid = rc.relnamespace
    WHERE c.conrelid IN (SELECT oid FROM rel)
      AND c.contype IN ('p', 'u', 'f', 'c', 'x')
    GROUP BY c.conrelid
),

idx AS (
    SELECT i.indrelid,
           jsonb_agg(jsonb_build_object(
               'name',       ic.relname,
               'unique',     i.indisunique,
               'partial',    i.indpred IS NOT NULL,
               'definition', pg_get_indexdef(i.indexrelid)
           ) ORDER BY ic.relname) AS indexes
    FROM pg_index i
    JOIN pg_class ic ON ic.oid = i.indexrelid
    WHERE i.indrelid IN (SELECT oid FROM rel)
      -- Indexes that implement PK / UNIQUE / EXCLUDE constraints are already
      -- described by the constraint; listing them twice would make emitted
      -- DDL create the same index name twice.
      AND NOT EXISTS (SELECT 1 FROM pg_constraint pc WHERE pc.conindid = i.indexrelid
                                                       AND pc.contype IN ('p', 'u', 'x'))
    GROUP BY i.indrelid
),

policies AS (
    SELECT p.polrelid,
           jsonb_agg(jsonb_build_object(
               'name',       p.polname,
               'command',    p.polcmd::text,
               'permissive', p.polpermissive,
               -- polroles = {0} means PUBLIC; reported as an empty list.
               'roles',      coalesce((SELECT jsonb_agg(pg_get_userbyid(r) ORDER BY r)
                                       FROM unnest(p.polroles) AS r WHERE r <> 0), '[]'::jsonb),
               'using',      pg_get_expr(p.polqual, p.polrelid),
               'check',      pg_get_expr(p.polwithcheck, p.polrelid)
           ) ORDER BY p.polname) AS policies
    FROM pg_policy p
    WHERE p.polrelid IN (SELECT oid FROM rel)
    GROUP BY p.polrelid
),

-- Which relations a view reads from (through its _RETURN rewrite rule). Needed
-- to order CREATE VIEW statements and to draw view -> table edges.
view_deps AS (
    SELECT r.ev_class AS view_oid,
           jsonb_agg(DISTINCT jsonb_build_object('schema', dn.nspname, 'name', dc.relname)) AS depends_on
    FROM pg_rewrite r
    JOIN pg_depend d     ON d.classid = 'pg_rewrite'::regclass AND d.objid = r.oid
                        AND d.refclassid = 'pg_class'::regclass AND d.refobjid <> r.ev_class
    JOIN pg_class dc     ON dc.oid = d.refobjid
    JOIN pg_namespace dn ON dn.oid = dc.relnamespace
    WHERE r.ev_class IN (SELECT oid FROM rel)
      AND r.rulename = '_RETURN'
    GROUP BY r.ev_class
)

SELECT jsonb_build_object(
    'remus_version',  '0.1',
    'generated_at',   now(),
    'database',       current_database(),
    'server_version', current_setting('server_version'),

    'entities', coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'schema',           r.schema,
            'name',             r.name,
            'kind',             r.kind,
            'comment',          r.comment,
            'rls',              r.rls,
            'rls_forced',       r.rls_forced,
            'unlogged',         r.unlogged,
            'partition_key',    r.partition_key,
            'view_definition',  r.view_definition,
            'depends_on',       coalesce(vd.depends_on, '[]'::jsonb),
            'columns',          coalesce(cl.columns, '[]'::jsonb),
            'constraints',      coalesce(cn.constraints, '[]'::jsonb),
            'indexes',          coalesce(ix.indexes, '[]'::jsonb),
            'policies',         coalesce(po.policies, '[]'::jsonb)
        ) ORDER BY r.schema, r.name)
        FROM rel r
        LEFT JOIN cols      cl ON cl.attrelid = r.oid
        LEFT JOIN cons      cn ON cn.conrelid = r.oid
        LEFT JOIN idx       ix ON ix.indrelid = r.oid
        LEFT JOIN policies  po ON po.polrelid = r.oid
        LEFT JOIN view_deps vd ON vd.view_oid = r.oid
    ), '[]'::jsonb),

    'enums', coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'schema',  n.nspname,
            'name',    t.typname,
            'labels',  coalesce((SELECT jsonb_agg(e.enumlabel ORDER BY e.enumsortorder)
                                 FROM pg_enum e WHERE e.enumtypid = t.oid), '[]'::jsonb),
            'comment', obj_description(t.oid, 'pg_type')
        ) ORDER BY n.nspname, t.typname)
        FROM pg_type t JOIN ns n ON n.oid = t.typnamespace
        WHERE t.typtype = 'e'
    ), '[]'::jsonb),

    'domains', coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'schema',      n.nspname,
            'name',        t.typname,
            'base_type',   format_type(t.typbasetype, t.typtypmod),
            'not_null',    t.typnotnull,
            'default',     t.typdefault,
            'constraints', coalesce((SELECT jsonb_agg(pg_get_constraintdef(c.oid) ORDER BY c.conname)
                                     FROM pg_constraint c WHERE c.contypid = t.oid), '[]'::jsonb),
            'comment',     obj_description(t.oid, 'pg_type')
        ) ORDER BY n.nspname, t.typname)
        FROM pg_type t JOIN ns n ON n.oid = t.typnamespace
        WHERE t.typtype = 'd'
    ), '[]'::jsonb),

    -- Stand-alone composite types (CREATE TYPE ... AS). Every table also has a
    -- row type with typtype = 'c'; those are excluded via relkind.
    'composites', coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'schema',     n.nspname,
            'name',       t.typname,
            'attributes', coalesce((SELECT jsonb_agg(jsonb_build_object(
                                        'name', a.attname,
                                        'type', format_type(a.atttypid, a.atttypmod)
                                    ) ORDER BY a.attnum)
                                    FROM pg_attribute a
                                    WHERE a.attrelid = t.typrelid AND a.attnum > 0
                                      AND NOT a.attisdropped), '[]'::jsonb),
            'comment',    obj_description(t.oid, 'pg_type')
        ) ORDER BY n.nspname, t.typname)
        FROM pg_type t
        JOIN ns n       ON n.oid = t.typnamespace
        JOIN pg_class c ON c.oid = t.typrelid
        WHERE t.typtype = 'c' AND c.relkind = 'c'
    ), '[]'::jsonb),

    'partitions', coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'parent_schema', pn.nspname,
            'parent',        pc.relname,
            'child_schema',  cn2.nspname,
            'child',         c.relname,
            'bound',         pg_get_expr(c.relpartbound, c.oid)
        ) ORDER BY pn.nspname, pc.relname, c.relname)
        FROM pg_class c
        JOIN pg_inherits inh  ON inh.inhrelid = c.oid
        JOIN pg_class pc      ON pc.oid = inh.inhparent
        JOIN ns pn            ON pn.oid = pc.relnamespace
        JOIN pg_namespace cn2 ON cn2.oid = c.relnamespace
        -- pg_inherits also links partitioned indexes to their partitions; only
        -- table partitions belong here.
        WHERE c.relispartition
          AND c.relkind IN ('r', 'p', 'f')
    ), '[]'::jsonb),

    'extensions', coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'name',    e.extname,
            'schema',  n.nspname,
            'version', e.extversion
        ) ORDER BY e.extname)
        FROM pg_extension e JOIN pg_namespace n ON n.oid = e.extnamespace
        WHERE e.extname <> 'plpgsql'
    ), '[]'::jsonb)
)::text
FROM cfg;
