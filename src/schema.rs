table! {
    api_tokens (id) {
        id -> Text,
        user_id -> Text,
        created_at -> Timestamp,
    }
}

table! {
    audiobooks (id) {
        id -> Text,
        location -> Text,
        title -> Varchar,
        artist -> Nullable<Varchar>,
        length -> Float8,
        library_id -> Text,
        hash -> Binary,
        file_extension -> Varchar,
        deleted -> Bool,
    }
}

table! {
    chapters (id) {
        id -> Text,
        title -> Nullable<Varchar>,
        audiobook_id -> Text,
        start_time -> Float8,
        number -> Int8,
    }
}

table! {
    libraries (id) {
        id -> Text,
        location -> Text,
        is_audiobook_regex -> Text,
        last_scan -> Nullable<Timestamp>,
    }
}

table! {
    library_permissions (library_id, user_id) {
        library_id -> Text,
        user_id -> Text,
    }
}

table! {
    playstates (audiobook_id, user_id) {
        audiobook_id -> Text,
        user_id -> Text,
        position -> Float8,
        timestamp -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        email -> Varchar,
        password_hash -> Varchar,
    }
}

joinable!(api_tokens -> users (user_id));
joinable!(audiobooks -> libraries (library_id));
joinable!(chapters -> audiobooks (audiobook_id));
joinable!(library_permissions -> libraries (library_id));
joinable!(library_permissions -> users (user_id));
joinable!(playstates -> audiobooks (audiobook_id));
joinable!(playstates -> users (user_id));

allow_tables_to_appear_in_same_query!(
    api_tokens,
    audiobooks,
    chapters,
    libraries,
    library_permissions,
    playstates,
    users,
);
