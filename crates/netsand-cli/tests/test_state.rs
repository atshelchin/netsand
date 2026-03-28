use netsand::state::{DaemonState, ProfilePort, ProcessInfo};

#[test]
fn test_state_get_port() {
    let state = DaemonState {
        pid: 1234,
        profiles: vec![
            ProfilePort { name: "bot".into(), port: 8001 },
            ProfilePort { name: "scraper".into(), port: 8002 },
        ],
        processes: vec![],
    };
    assert_eq!(state.get_port("bot"), Some(8001));
    assert_eq!(state.get_port("scraper"), Some(8002));
    assert_eq!(state.get_port("unknown"), None);
}

#[test]
fn test_state_save_load() {
    // Use a temp dir to avoid polluting real state
    let tmp = std::env::temp_dir().join("netsand-test-state");
    std::fs::create_dir_all(&tmp).ok();
    let state_file = tmp.join("test-state.json");

    let state = DaemonState {
        pid: 9999,
        profiles: vec![ProfilePort { name: "test".into(), port: 8888 }],
        processes: vec![ProcessInfo {
            pid: 1111,
            profile: "test".into(),
            command: "echo hello".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            proxy_port: 8888,
        }],
    };

    let json = serde_json::to_string_pretty(&state).unwrap();
    std::fs::write(&state_file, &json).unwrap();

    let loaded: DaemonState = serde_json::from_str(
        &std::fs::read_to_string(&state_file).unwrap()
    ).unwrap();

    assert_eq!(loaded.pid, 9999);
    assert_eq!(loaded.profiles.len(), 1);
    assert_eq!(loaded.profiles[0].name, "test");
    assert_eq!(loaded.processes.len(), 1);
    assert_eq!(loaded.processes[0].pid, 1111);

    std::fs::remove_dir_all(&tmp).ok();
}
