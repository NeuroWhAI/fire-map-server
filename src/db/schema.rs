table! {
    reports (id) {
        id -> Integer,
        user_id -> Text,
        user_pwd -> Text,
        latitude -> Double,
        longitude -> Double,
        created_time -> Timestamp,
        lvl -> Integer,
        description -> Text,
        img_path -> Text,
    }
}

table! {
    bad_reports (id) {
        id -> Integer,
        report_id -> Integer,
        reason -> Text,
    }
}