use hmail_cleaner_protocol::delivery::{CURRENT_STORAGE_SCHEMA, SUPPORTED_BACKUP_SCHEMAS};
fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "current_schema_version": CURRENT_STORAGE_SCHEMA,
            "backup_schema_versions": SUPPORTED_BACKUP_SCHEMAS
        }))
        .unwrap()
    );
}
