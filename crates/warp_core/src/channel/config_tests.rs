use super::{OzConfig, WarpServerConfig};

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
}
