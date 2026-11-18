use super::*;

#[test]
fn github_cli_reads_require_explicit_host_local_opt_in() {
    assert!(!serve(&["serve", "--state-dir", "/s"]).github_cli);
    assert!(serve(&["serve", "--state-dir", "/s", "--github-cli"]).github_cli);
    assert!(USAGE.contains("--github-cli"));
    let config: crate::app::config::RemoteConfig = toml::from_str("github_cli = true").unwrap();
    assert!(config.github_cli);
    assert!(!crate::app::config::RemoteConfig::default().github_cli);
}

#[test]
fn enterprise_hosts_come_from_the_config_as_named_hosts() {
    let options = serve(&[
        "serve",
        "--state-dir",
        "/s",
        "--github-host",
        "ghe.example.com",
        "--github-host",
        "git.corp.example",
    ]);
    assert_eq!(
        options.github_hosts,
        ["ghe.example.com", "git.corp.example"]
    );
    assert!(
        serve(&["serve", "--state-dir", "/s"])
            .github_hosts
            .is_empty()
    );
    assert!(USAGE.contains("--github-host"));
    for bad in ["https://ghe.example.com", "ghe.example.com:8443", "GHE"] {
        let parsed = parse(args(&["serve", "--state-dir", "/s", "--github-host", bad]));
        assert_eq!(
            parsed.err(),
            Some(ParseError::Value {
                option: "--github-host"
            }),
            "{bad}"
        );
    }
    let config: crate::app::config::RemoteConfig =
        toml::from_str(r#"github_hosts = ["ghe.example.com"]"#).unwrap();
    assert_eq!(config.github_hosts, ["ghe.example.com"]);
    assert!(config.check().is_ok());
    let bad: crate::app::config::RemoteConfig =
        toml::from_str(r#"github_hosts = ["https://ghe.example.com"]"#).unwrap();
    assert!(matches!(
        bad.check(),
        Err(crate::app::Error::GitHubHost { host }) if host == "https://ghe.example.com"
    ));
}
