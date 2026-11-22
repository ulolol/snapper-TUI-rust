//! Read-only integration probe against a real local Snapper 0.13.2 daemon/CLI.
//! Run explicitly on Linux via:
//! `cargo test -p snapper-core --features dbus --test live_readonly -- --ignored --nocapture`

#[cfg(all(target_os = "linux", feature = "dbus"))]
#[test]
#[ignore]
fn live_read_only_snapper_probe() {
    use snapper_core::backend::{
        ElevationStrategy, SnapperBackend, SnapperCliBackend, SnapperDbusBackend,
    };

    let dbus = SnapperDbusBackend::connect().expect("connect to org.opensuse.Snapper D-Bus");
    let dbus_configs = dbus.list_configs().expect("D-Bus list_configs");
    println!("D-Bus configs ({}): {:?}", dbus_configs.len(), dbus_configs);
    assert!(
        !dbus_configs.is_empty(),
        "expected at least one snapper config on live Linux host"
    );

    let dbus_snaps = dbus
        .list_snapshots(None, true)
        .expect("D-Bus list_snapshots(None, true)");
    println!("D-Bus snapshots total: {}", dbus_snaps.len());
    assert!(!dbus_snaps.is_empty());

    let first_cfg = &dbus_configs[0].name;
    let dbus_settings = dbus.get_config(first_cfg).expect("D-Bus get_config");
    println!(
        "D-Bus config '{first_cfg}' settings count: {}",
        dbus_settings.len()
    );
    assert!(!dbus_settings.is_empty());

    if let Some(sample) = dbus_snaps.iter().find(|s| s.config == *first_cfg && s.number > 0)
        && let Ok(changes) = dbus.snapshot_status(first_cfg, sample.number, 0)
    {
        println!(
            "D-Bus snapshot_status({first_cfg}, {}..0): {} changed paths",
            sample.number,
            changes.len()
        );
    }

    let cli = SnapperCliBackend::new(ElevationStrategy::Direct);
    match cli.list_configs() {
        Ok(cli_configs) => {
            println!("CLI direct configs: {}", cli_configs.len());
            assert_eq!(cli_configs.len(), dbus_configs.len());
            let cli_snaps = cli
                .list_snapshots(None, false)
                .expect("CLI list_snapshots(None, false)");
            println!("CLI direct snapshots total: {}", cli_snaps.len());
        }
        Err(err) => {
            println!("CLI direct requires root/ACL (expected for non-root): {err}");
        }
    }
}
