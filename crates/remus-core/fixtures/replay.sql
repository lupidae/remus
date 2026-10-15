-- Objects whose SQL output only replays in the right order, or with the
-- parentheses pg_get_expr leaves out. `just roundtrip` loads this, renders
-- `-f sql`, replays that with ON_ERROR_STOP and requires the same model back.
-- Every statement here is valid on PostgreSQL 12+.
--
-- Two cases are absent because PostgreSQL refuses them: a foreign key over a
-- partial unique index, and any index on a plain view.

-- lupidae/remus#16, verbatim.
CREATE TABLE public.docs (id int PRIMARY KEY, owner text NOT NULL, code text NOT NULL);
ALTER TABLE public.docs ENABLE ROW LEVEL SECURITY;
CREATE POLICY all_read ON public.docs FOR SELECT USING (true);
CREATE UNIQUE INDEX docs_code_key ON public.docs (code);
CREATE TABLE public.refs (doc_code text REFERENCES public.docs (code));
CREATE MATERIALIZED VIEW public.doc_stats AS SELECT owner, count(*) AS n FROM public.docs GROUP BY owner;
CREATE UNIQUE INDEX doc_stats_owner ON public.doc_stats (owner);

-- A unique index with INCLUDE columns can back a foreign key too.
ALTER TABLE public.docs ADD COLUMN revision int NOT NULL DEFAULT 1;
CREATE UNIQUE INDEX docs_id_revision_key ON public.docs (id, revision) INCLUDE (owner);
CREATE TABLE public.annotations (
    doc_id   int NOT NULL,
    revision int NOT NULL,
    body     text,
    FOREIGN KEY (doc_id, revision) REFERENCES public.docs (id, revision) ON DELETE CASCADE
);

-- A materialized view over a materialized view, each indexed.
CREATE MATERIALIZED VIEW public.busy_owners AS SELECT owner FROM public.doc_stats WHERE n > 10;
CREATE INDEX busy_owners_owner ON public.busy_owners (owner);

-- An index on a partitioned table deparses as `ON ONLY`, which is only valid
-- before the partitions exist; a foreign key over it fails otherwise. The
-- partition is partitioned in turn, so the index has two levels to reach.
CREATE TABLE public.ledger (
    id        bigint NOT NULL,
    booked_on date NOT NULL,
    amount    numeric NOT NULL
) PARTITION BY RANGE (booked_on);
CREATE TABLE public.ledger_2026 PARTITION OF public.ledger
    FOR VALUES FROM ('2026-01-01') TO ('2027-01-01') PARTITION BY HASH (id);
CREATE TABLE public.ledger_2026_h0 PARTITION OF public.ledger_2026
    FOR VALUES WITH (MODULUS 2, REMAINDER 0);
CREATE TABLE public.ledger_2026_h1 PARTITION OF public.ledger_2026
    FOR VALUES WITH (MODULUS 2, REMAINDER 1);
CREATE UNIQUE INDEX ledger_entry_key ON public.ledger (id, booked_on);
CREATE INDEX ledger_amount_idx ON public.ledger (amount);
CREATE TABLE public.ledger_notes (
    entry_id  bigint NOT NULL,
    booked_on date NOT NULL,
    FOREIGN KEY (entry_id, booked_on) REFERENCES public.ledger (id, booked_on)
);

-- Policy expressions pg_get_expr does not wrap: a constant, a column, a
-- function call, a CASE. The others come back wrapped already, one with
-- parentheses inside its string literals.
ALTER TABLE public.docs ADD COLUMN published boolean;
CREATE POLICY published_read ON public.docs FOR SELECT USING (published);
CREATE POLICY coded_read ON public.docs FOR SELECT USING (starts_with(code, 'A'));
CREATE POLICY never_write ON public.docs FOR INSERT WITH CHECK (false);
CREATE POLICY owner_update ON public.docs FOR UPDATE
    USING (owner = CURRENT_USER)
    WITH CHECK (CASE WHEN published THEN owner = CURRENT_USER ELSE true END);
CREATE POLICY bracketed ON public.docs AS RESTRICTIVE FOR DELETE
    USING (code <> '(' AND code <> ')');
ALTER TABLE public.refs ENABLE ROW LEVEL SECURITY;
CREATE POLICY refs_read ON public.refs USING (coalesce(doc_code <> '', false));

-- NOT VALID survives only ALTER: CREATE TABLE validates the check anyway and
-- CREATE DOMAIN rejects it. On the partitioned table it must stay on the parent.
ALTER TABLE public.refs ADD CONSTRAINT refs_code_upper CHECK (doc_code = upper(doc_code)) NOT VALID;
ALTER TABLE public.ledger ADD CONSTRAINT ledger_amount_nonzero CHECK (amount <> 0) NOT VALID;

-- A domain default deparsed under the creator's search_path names its type
-- unqualified in pg_type.typdefault. The NOT VALID check sorts before the valid
-- one, which a replay must keep.
CREATE SCHEMA library;
CREATE TYPE library.shelf AS ENUM ('new', 'archive');
SET search_path = library;
CREATE DOMAIN library.placement AS shelf DEFAULT 'new'
    CONSTRAINT placement_known CHECK (VALUE IS NOT NULL);
ALTER DOMAIN placement ADD CONSTRAINT placement_active CHECK (VALUE <> 'archive') NOT VALID;
RESET search_path;
CREATE TABLE library.loans (
    id    int PRIMARY KEY,
    shelf library.placement
);
