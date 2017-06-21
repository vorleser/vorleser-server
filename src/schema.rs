//infer_schema!("dotenv:DATABASE_URL");
table! {
    api_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        created_at -> Timestamp,
    }
}

table! {
    audiobooks (id) {
        id -> Uuid,
        location -> Text,
        mime_type -> Varchar,
        title -> Varchar,
        length -> Float8,
        library_id -> Uuid,
        hash -> Bytea,
    }
}

table! {
    chapters (id) {
        id -> Uuid,
        title -> Nullable<Varchar>,
        audiobook_id -> Uuid,
        start_time -> Float8,
        number -> Int8,
    }
}

table! {
    libraries (id) {
        id -> Uuid,
        content_change_date -> Timestamp,
        location -> Text,
        is_audiobook_regex -> Text,
        last_scan -> Nullable<Timestamp>,
    }
}

table! {
    library_permissions (library_id,
    user_id) {
        library_id -> Uuid,
        user_id -> Uuid,
    }
}

table! {
    playstates (audiobook_id,
    user_id) {
        audiobook_id -> Uuid,
        user_id -> Uuid,
        completed -> Bool,
        position -> Float8,
        timestamp -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        email -> Varchar,
        password_hash -> Varchar,
    }
}
