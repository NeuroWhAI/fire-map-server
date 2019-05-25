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

table! {
    shelters (id) {
        id -> Integer,
        name -> Text,
        latitude -> Double,
        longitude -> Double,
        info -> Text,
        recent_good -> Integer,
        recent_bad -> Integer,
    }
}

table! {
    user_shelters (id) {
        id -> Integer,
        name -> Text,
        latitude -> Double,
        longitude -> Double,
        info -> Text,
        evidence -> Text,
    }
}