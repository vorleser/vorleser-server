CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP DEFAULT current_timestamp NOT NULL,
    updated_at TIMESTAMP DEFAULT current_timestamp NOT NULL,
    email VARCHAR(120) UNIQUE NOT NULL,
    password_hash VARCHAR(120) NOT NULL
);
SELECT diesel_manage_updated_at('users');

CREATE TABLE api_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users (id) NOT NULL,
    created_at TIMESTAMP DEFAULT current_timestamp NOT NULL
);

CREATE TABLE libraries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    content_change_date TIMESTAMP DEFAULT current_timestamp NOT NULL,
    location TEXT NOT NULL,
    is_audiobook_regex TEXT NOT NULL,
    last_scan TIMESTAMP
);

CREATE TABLE library_access (
    library_id UUID REFERENCES libraries (id),
    user_id UUID REFERENCES users (id),
    PRIMARY KEY(library_id, user_id)
);

CREATE TABLE audiobooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    location TEXT NOT NULL, -- rename to source location
    -- mime_type VARCHAR(255) NOT NULL,
    -- hash
    title VARCHAR(1024) NOT NULL,
    length DOUBLE PRECISION NOT NULL,
    library_id UUID REFERENCES libraries (id) NOT NULL,
    hash BYTEA NOT NULL
);

CREATE TABLE chapters (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(1024),
    audiobook_id UUID REFERENCES audiobooks (id) NOT NULL,
    start_time DOUBLE PRECISION NOT NULL,
    number BIGINT NOT NULL
);

CREATE TABLE playstates (
    audiobook_id UUID REFERENCES audiobooks (id),
    user_id UUID REFERENCES users (id),
    completed BOOL NOT NULL,
    position DOUBLE PRECISION NOT NULL,
    PRIMARY KEY(audiobook_id, user_id)
);
