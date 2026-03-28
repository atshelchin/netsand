use std::path::{Path, PathBuf};

fn profiles_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/netsand-cli/
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap() // crates/
        .parent().unwrap() // workspace root
        .join("profiles")
}

#[test]
fn test_load_bot_profile() {
    let p = netsand::profile::Profile::load(&profiles_dir().join("bot.toml"), 9999).unwrap();
    assert_eq!(p.name, "bot");
    assert_eq!(p.listen_port, 8001);
    assert!(p.policy.is_allowed("gamma-api.polymarket.com"));
    assert!(p.policy.is_allowed("stream.binance.com"));
    assert!(!p.policy.is_allowed("evil.com"));
    assert!(p.policy.needs_upstream("gamma-api.polymarket.com"));
    assert!(!p.policy.needs_upstream("stream.binance.com"));
    assert_eq!(p.sandbox_user.as_deref(), Some("botuser"));
}

#[test]
fn test_load_scraper_profile() {
    let p = netsand::profile::Profile::load(&profiles_dir().join("scraper.toml"), 9999).unwrap();
    assert_eq!(p.name, "scraper");
    assert_eq!(p.listen_port, 8002);
    assert!(p.policy.is_allowed("api.coingecko.com"));
    assert!(!p.policy.is_allowed("evil.com"));
    assert!(p.policy.upstream.is_none());
    assert!(p.sandbox_user.is_none());
}

#[test]
fn test_load_all_profiles() {
    let profiles = netsand::profile::load_all(&profiles_dir()).unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].name, "bot");
    assert_eq!(profiles[1].name, "scraper");
}

#[test]
fn test_default_port_assignment() {
    let p = netsand::profile::Profile::load(&profiles_dir().join("scraper.toml"), 7777).unwrap();
    assert_eq!(p.listen_port, 8002); // explicit wins over default
}

#[test]
fn test_invalid_profile_path() {
    assert!(netsand::profile::Profile::load(Path::new("nonexistent.toml"), 8001).is_err());
}

#[test]
fn test_invalid_profile_dir() {
    assert!(netsand::profile::load_all(Path::new("/nonexistent/dir")).is_err());
}
