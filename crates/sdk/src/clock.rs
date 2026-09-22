/// Machine time and identifiers. Leaves receive the strings these produce.

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}
