use super::{ChannelConfig, OzConfig, TelemetryConfig, WarpServerConfig};
use crate::AppId;

#[test]
fn disabled_server_configuration_never_targets_warp_services() {
    let server = WarpServerConfig::disabled();
    let oz = OzConfig::disabled();

    for value in [
        &server.server_root_url[..],
        &server.rtc_server_url[..],
        &oz.oz_root_url[..],
    ] {
        assert!(value.contains("127.0.0.1:9"));
        assert!(!value.contains("warp.dev"));
    }
    assert!(server.session_sharing_server_url.is_none());
    assert!(server.firebase_auth_api_key.is_empty());
    assert!(
        WarpServerConfig::production()
            .firebase_auth_api_key
            .is_empty()
    );
}

#[test]
fn without_warp_services_strips_cloud_and_telemetry() {
    let config = ChannelConfig {
        app_id: AppId::new("dev", "warp", "Warp"),
        logfile_name: "warp.log".into(),
        server_config: WarpServerConfig::production(),
        oz_config: OzConfig::production(),
        telemetry_config: Some(TelemetryConfig {
            telemetry_file_name: "telemetry".into(),
            rudderstack_config: None,
        }),
        autoupdate_config: None,
        crash_reporting_config: None,
        mcp_static_config: None,
    }
    .without_warp_services();

    assert!(config.telemetry_config.is_none());
    assert!(config.crash_reporting_config.is_none());
    assert!(config.autoupdate_config.is_none());
    assert!(config.server_config.server_root_url.contains("127.0.0.1:9"));
    assert!(config.oz_config.oz_root_url.contains("127.0.0.1:9"));
    assert!(config.server_config.firebase_auth_api_key.is_empty());
}
