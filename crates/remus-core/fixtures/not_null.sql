-- NOT NULL wherever the catalog can record it. PostgreSQL 17 also catalogues a
-- domain's NOT NULL in pg_constraint, and 18 a column's; the model must not
-- notice. `just roundtrips` replays this on every major it is given and requires
-- the same model from each. Every statement here is valid on PostgreSQL 12+.

-- lupidae/remus#13, verbatim.
CREATE DOMAIN public.email AS text NOT NULL CHECK (VALUE ~ '@');
CREATE TABLE public.u (e public.email);

-- A named NOT NULL (17+ keeps the name, earlier majors drop it) and two CHECKs.
CREATE DOMAIN public.positive AS integer
    CONSTRAINT positive_not_null NOT NULL
    CHECK (VALUE > 0)
    CHECK (VALUE < 1000000);
CREATE DOMAIN public.code AS text CHECK (VALUE ~ '^[A-Z]+$');

CREATE TABLE public.accounts (
    id       bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    email    public.email,
    name     text CONSTRAINT accounts_name_required NOT NULL,
    nickname text,
    balance  numeric NOT NULL DEFAULT 0 CHECK (balance >= 0),
    code     public.code,
    CONSTRAINT accounts_name_not_blank CHECK (name <> '')
);

-- NOT NULL on a partitioned table, which every partition inherits.
CREATE TABLE public.readings (
    account_id bigint NOT NULL REFERENCES public.accounts (id),
    taken_at   timestamptz NOT NULL,
    value      public.positive,
    note       text CHECK (note <> ''),
    PRIMARY KEY (account_id, taken_at)
) PARTITION BY RANGE (taken_at);
CREATE TABLE public.readings_2026 PARTITION OF public.readings
    FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');
CREATE TABLE public.readings_default PARTITION OF public.readings DEFAULT;

-- Inheritance, which the model does not carry: the child is a table of its
-- own, with the NOT NULLs and CHECKs it inherited.
CREATE TABLE public.vehicles (
    id     bigint PRIMARY KEY,
    wheels integer NOT NULL CHECK (wheels > 0),
    label  text
);
CREATE TABLE public.cars (
    doors integer NOT NULL CHECK (doors > 0)
) INHERITS (public.vehicles);
