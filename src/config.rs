use std::{collections::HashSet, env};

#[derive(Clone)]
pub struct Config {
    pub bind_addr: String,
    pub token: String,

    pub upload_dir: String,
    pub max_upload_bytes: usize,
    pub body_limit_bytes: usize,
    pub allowed_mimes: HashSet<String>,

    /// 0 => unlimited (do not delete)
    pub message_limit: usize,
}

fn get_env(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn parse_usize_env(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

pub fn load_config() -> Config {
    let bind_addr = get_env("BIND_ADDR", "127.0.0.1:3000");
    let token = get_env("TOKEN", "token");

    let upload_dir = get_env("UPLOAD_DIR", "static/uploads");
    let max_upload_bytes = parse_usize_env("MAX_UPLOAD_BYTES", 10 * 1024 * 1024);
    let body_limit_bytes = parse_usize_env("BODY_LIMIT_BYTES", 20 * 1024 * 1024);

    let allowed_raw = get_env(
        "ALLOWED_MIMES",
        "image/jpeg,image/png,image/webp,image/gif,application/pdf,application/zip",
    );
    let allowed_mimes: HashSet<String> = allowed_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let message_limit = parse_usize_env("MESSAGE_LIMIT", 50);

    Config {
        bind_addr,
        token,
        upload_dir,
        max_upload_bytes,
        body_limit_bytes,
        allowed_mimes,
        message_limit,
    }
}
