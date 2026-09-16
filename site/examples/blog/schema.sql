-- A small blog: enough to show keys, cardinalities, an enum, a junction table
-- and a view, without the noise of a real application schema.

CREATE TYPE post_status AS ENUM ('draft', 'published', 'archived');

CREATE TABLE users (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    email      text NOT NULL UNIQUE,
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
COMMENT ON TABLE users IS 'Anyone who can log in';

CREATE TABLE profiles (
    user_id bigint PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    bio     text,
    website text
);

CREATE TABLE posts (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    author_id    bigint NOT NULL REFERENCES users (id),
    title        text NOT NULL,
    slug         text NOT NULL UNIQUE,
    body         text NOT NULL,
    status       post_status NOT NULL DEFAULT 'draft',
    published_at timestamptz,
    CONSTRAINT posts_published_have_a_date CHECK (status <> 'published' OR published_at IS NOT NULL)
);
CREATE INDEX posts_author_idx ON posts (author_id);
CREATE INDEX posts_published_idx ON posts (published_at DESC) WHERE status = 'published';
COMMENT ON COLUMN posts.slug IS 'URL fragment, unique per site';

CREATE TABLE comments (
    id        bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    post_id   bigint NOT NULL REFERENCES posts (id) ON DELETE CASCADE,
    author_id bigint REFERENCES users (id) ON DELETE SET NULL,
    parent_id bigint REFERENCES comments (id) ON DELETE CASCADE,
    body      text NOT NULL,
    posted_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE tags (
    id   integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name text NOT NULL UNIQUE
);

-- Pure junction table: --conceptual collapses it into posts }o--o{ tags.
CREATE TABLE post_tags (
    post_id bigint  NOT NULL REFERENCES posts (id) ON DELETE CASCADE,
    tag_id  integer NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);

CREATE VIEW published_posts AS
    SELECT p.id, p.title, p.slug, u.name AS author, p.published_at
    FROM posts p
    JOIN users u ON u.id = p.author_id
    WHERE p.status = 'published';
