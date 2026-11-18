use super::*;

#[test]
fn service_only_passes_github_permission_when_explicitly_enabled() {
    let mut plan = plan();
    assert!(!plan.args().iter().any(|arg| arg == "--github-cli"));
    plan.config.github_cli = true;
    assert!(plan.args().iter().any(|arg| arg == "--github-cli"));
    plan.config.github_cli = false;
    assert!(!plan.args().iter().any(|arg| arg == "--github-cli"));
}

#[test]
fn service_passes_each_enterprise_host() {
    let mut plan = plan();
    assert!(!plan.args().iter().any(|arg| arg == "--github-host"));
    plan.config.github_hosts = vec!["ghe.example.com".into(), "git.corp.example".into()];
    let args = plan.args();
    let hosts: Vec<_> = args
        .windows(2)
        .filter(|pair| pair[0] == "--github-host")
        .map(|pair| pair[1].clone())
        .collect();
    assert_eq!(hosts, ["ghe.example.com", "git.corp.example"]);
}
