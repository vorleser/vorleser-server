CREATE TABLE users (
    id VARCHAR(36) PRIMARY KEY,
    created_at TIMESTAMP DEFAULT current_timestamp NOT NULL,
    updated_at TIMESTAMP DEFAULT current_timestamp NOT NULL,
    email VARCHAR(120) UNIQUE NOT NULL,
    password_hash VARCHAR(240) NOT NULL
);

CREATE TABLE api_tokens (
    id VARCHAR(36) PRIMARY KEY,
    user_id VARCHAR(36) REFERENCES users (id) NOT NULL,
    created_at TIMESTAMP DEFAULT current_timestamp NOT NULL
);

CREATE TABLE libraries (
    id VARCHAR(36) PRIMARY KEY,
    location TEXT NOT NULL,
    is_audiobook_regex TEXT NOT NULL,
    last_scan TIMESTAMP
);

CREATE TABLE library_permissions (
    library_id VARCHAR(36) REFERENCES libraries (id),
    user_id VARCHAR(36) REFERENCES users (id),
    PRIMARY KEY(library_id, user_id)
);

CREATE TABLE audiobooks (
    id VARCHAR(36) PRIMARY KEY,
    location TEXT NOT NULL, -- rename to source location
    title VARCHAR(1024) NOT NULL,
    artist VARCHAR(1024),
    length DOUBLE PRECISION NOT NULL,
    library_id UUID REFERENCES libraries (id) NOT NULL,
    hash BYTEA NOT NULL,
    file_extension VARCHAR(255) NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE chapters (
    id VARCHAR(36) PRIMARY KEY,
    title VARCHAR(1024),
    audiobook_id UUID REFERENCES audiobooks (id) NOT NULL,
    start_time DOUBLE PRECISION NOT NULL,
    number BIGINT NOT NULL
);

CREATE TABLE playstates (
    audiobook_id VARCHAR(36),
    user_id VARCHAR(36) REFERENCES users (id),
    position DOUBLE PRECISION NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    PRIMARY KEY(audiobook_id, user_id)
);
