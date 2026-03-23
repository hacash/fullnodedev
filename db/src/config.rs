use std::sync::OnceLock;

fn db_env_enable(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn db_sync_enabled() -> bool {
    static DB_SYNC: OnceLock<bool> = OnceLock::new();
    *DB_SYNC.get_or_init(|| db_env_enable("HACASH_DB_SYNC"))
}

fn db_sled_small_machine_enabled() -> bool {
    static DB_SLED_SMALL_MACHINE: OnceLock<bool> = OnceLock::new();
    *DB_SLED_SMALL_MACHINE.get_or_init(|| 
        db_env_enable("HACASH_DB_SMALL_MACHINE"))
}
