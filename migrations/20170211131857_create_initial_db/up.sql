CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    created_at TIMESTAMP DEFAULT current_timestamp NOT NULL,
    updated_at TIMESTAMP DEFAULT current_timestamp NOT NULL,
    email VARCHAR(120) UNIQUE NOT NULL,
    password_hash VARCHAR(120) NOT NULL
);
SELECT diesel_manage_updated_at('users');

CREATE TABLE audiobooks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(1024) NOT NULL,
    length DOUBLE PRECISION NOT NULL
);

CREATE TABLE chapters (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(1024),
    audiobook_id UUID REFERENCES audiobooks (id),
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
