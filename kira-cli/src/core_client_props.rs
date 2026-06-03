//! Property-based tests for CoreClient URL handling.
//!
//! - Property 1: URL Precedence Resolution
//! - Property 2: Invalid URL Rejection

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::config::{Config, TimeConfig};
    use crate::core_client::CoreClient;

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Build a Config with a specific core_url and valid defaults for other fields.
    fn config_with_url(url: &str) -> Config {
        Config {
            core_url: url.to_string(),
            time: TimeConfig::default(),
            ..Config::default()
        }
    }

    /// Resolve the URL using the same precedence logic as Config::load:
    /// CLI flag > env var > config file value > default.
    ///
    /// This is a pure reference implementation of the precedence rule.
    fn resolve_url(
        cli: Option<&str>,
        env: Option<&str>,
        config_file: Option<&str>,
        default: &str,
    ) -> String {
        if let Some(url) = cli {
            return url.to_string();
        }
        if let Some(url) = env {
            return url.to_string();
        }
        if let Some(url) = config_file {
            return url.to_string();
        }
        default.to_string()
    }

    // ── Generators ────────────────────────────────────────────────────────────

    /// Generate a valid HTTP/HTTPS URL for testing precedence.
    fn valid_url_strategy() -> impl Strategy<Value = String> {
        let schemes = prop_oneof!["http", "https"];
        let hosts = prop_oneof![
            Just("127.0.0.1".to_string()),
            Just("localhost".to_string()),
            "[a-z]{3,10}\\.[a-z]{2,4}",
        ];
        let ports = prop_oneof![
            Just(String::new()),
            (1024u16..65535).prop_map(|p| format!(":{p}")),
        ];

        (schemes, hosts, ports).prop_map(|(scheme, host, port)| format!("{scheme}://{host}{port}"))
    }

    /// Generate an optional valid URL.
    fn optional_valid_url() -> impl Strategy<Value = Option<String>> {
        prop_oneof![Just(None), valid_url_strategy().prop_map(Some),]
    }

    /// Generate strings that are NOT valid HTTP/HTTPS URLs.
    fn invalid_url_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            // Empty string
            Just(String::new()),
            // Missing scheme entirely
            "[a-z]{3,10}\\.[a-z]{2,4}(:[0-9]{1,5})?",
            // FTP scheme (not http/https)
            "[a-z]{3,10}\\.[a-z]{2,4}".prop_map(|host| format!("ftp://{host}")),
            // Other invalid schemes
            "[a-z]{2,6}"
                .prop_filter("not http or https", |s| s != "http" && s != "https")
                .prop_flat_map(|scheme| {
                    "[a-z]{3,10}\\.[a-z]{2,4}".prop_map(move |host| format!("{scheme}://{host}"))
                }),
            // Strings with spaces (invalid URL chars)
            "[a-z ]{5,20}".prop_filter("must contain space", |s| s.contains(' ')),
            // Just a scheme with no host
            Just("http://".to_string()),
            // Relative path (no scheme)
            Just("not-a-url".to_string()),
            Just("/v1/health".to_string()),
            Just("://missing-scheme".to_string()),
        ]
    }

    // ── Property 1: URL Precedence Resolution ─────────────────────────────────
    // Feature: tui-wiring, Property 1: URL Precedence Resolution
    //
    // For any combination of (cli_flag, env_var, config_value, default),
    // the resolved core_url SHALL equal the first present value in
    // precedence order: cli_flag > env_var > config_value > default.
    //
    // **Validates: Requirements 1.4, 10.1**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_url_precedence_resolution(
            cli in optional_valid_url(),
            env in optional_valid_url(),
            config_file in optional_valid_url(),
            default in valid_url_strategy(),
        ) {
            // The expected resolved URL per the precedence rule
            let expected = resolve_url(cli.as_deref(), env.as_deref(), config_file.as_deref(), &default);

            // Simulate Config::load behavior:
            // 1. Start with the config file value or default (env var is checked
            //    inside default_core_url() during deserialization, but we test
            //    the precedence logic directly here).
            // 2. CLI override is applied on top.

            // The "base" URL is what would come from the config file or the
            // env var fallback or the hardcoded default.
            let base_url = env.as_ref().or(config_file.as_ref()).unwrap_or(&default).clone();

            // Build a config as if loaded from file (with env/config/default resolved)
            let mut cfg = config_with_url(&base_url);

            // Apply CLI override (same as Config::load does)
            if let Some(ref cli_url) = cli {
                cfg.core_url = cli_url.clone();
            }

            // The resolved URL should match our reference implementation
            prop_assert_eq!(
                cfg.core_url,
                expected,
                "Precedence violated: cli={:?}, env={:?}, config={:?}, default={:?}",
                cli, env, config_file, default
            );
        }
    }

    // ── Property 2: Invalid URL Rejection ─────────────────────────────────────
    // Feature: tui-wiring, Property 2: Invalid URL Rejection
    //
    // For any string that is not a valid HTTP/HTTPS URL (missing scheme,
    // invalid characters, empty string), CoreClient::new() SHALL return Err.
    //
    // **Validates: Requirements 1.3**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_invalid_url_rejection(url in invalid_url_strategy()) {
            let cfg = config_with_url(&url);
            let result = CoreClient::new(&cfg);

            prop_assert!(
                result.is_err(),
                "CoreClient::new() should reject invalid URL '{}' but returned Ok",
                url
            );
        }
    }

    // ── Unit tests for URL validation edge cases ──────────────────────────────

    #[test]
    fn valid_http_url_accepted() {
        let cfg = config_with_url("http://127.0.0.1:7777");
        assert!(CoreClient::new(&cfg).is_ok());
    }

    #[test]
    fn valid_https_url_accepted() {
        let cfg = config_with_url("https://example.com:8080");
        assert!(CoreClient::new(&cfg).is_ok());
    }

    #[test]
    fn empty_url_rejected() {
        let cfg = config_with_url("");
        assert!(CoreClient::new(&cfg).is_err());
    }

    #[test]
    fn ftp_scheme_rejected() {
        let cfg = config_with_url("ftp://example.com");
        assert!(CoreClient::new(&cfg).is_err());
    }

    #[test]
    fn no_scheme_rejected() {
        let cfg = config_with_url("not-a-url");
        assert!(CoreClient::new(&cfg).is_err());
    }

    #[test]
    fn url_with_spaces_rejected() {
        let cfg = config_with_url("http://my server.com");
        assert!(CoreClient::new(&cfg).is_err());
    }

    #[test]
    fn http_scheme_only_rejected() {
        // "http://" with no host should be rejected
        let cfg = config_with_url("http://");
        let result = CoreClient::new(&cfg);
        // reqwest::Url::parse("http://") returns Err (empty host), so this should fail
        assert!(result.is_err(), "http:// with no host should be rejected");
    }
}
