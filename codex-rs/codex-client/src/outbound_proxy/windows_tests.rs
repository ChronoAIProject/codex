use super::*;

#[test]
fn missing_ie_proxy_config_does_not_force_autodetect() {
    let config = ie_proxy_config_from_error(ERROR_FILE_NOT_FOUND)
        .expect("missing IE proxy settings should be accepted");

    assert_eq!(config, IeProxyConfig::default());
}

#[test]
fn proxy_bypass_matches_whitespace_separated_winhttp_entries() {
    let local_origin = RequestOrigin {
        scheme: "https".to_string(),
        host: "intranet".to_string(),
        port: 443,
    };
    assert!(proxy_bypass_matches_origin("<local> *.corp", &local_origin));

    let corp_origin = RequestOrigin {
        scheme: "https".to_string(),
        host: "service.corp".to_string(),
        port: 443,
    };
    assert!(proxy_bypass_matches_origin("<local> *.corp", &corp_origin));
}
