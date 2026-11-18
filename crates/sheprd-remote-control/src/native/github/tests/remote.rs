use crate::native::github::remote::{Repository, parse};

fn repository(host: &str, owner: &str, repo: &str) -> Option<Repository> {
    Some(Repository {
        host: host.into(),
        owner: owner.into(),
        repo: repo.into(),
    })
}

#[test]
fn remote_urls_name_their_host_and_repository() {
    let cases = [
        // scp-style SSH.
        (
            "git@github.com:sheprd-labs/sheprd.git",
            repository("github.com", "sheprd-labs", "sheprd"),
        ),
        ("github.com:o/r", repository("github.com", "o", "r")),
        ("git@GitHub.com:o/r", repository("github.com", "o", "r")),
        // ssh://, with a user and an SSH port, which says nothing of the API's.
        (
            "ssh://git@github.com/o/r.git",
            repository("github.com", "o", "r"),
        ),
        (
            "ssh://git@ghe.example.com:2222/team/app.git",
            repository("ghe.example.com", "team", "app"),
        ),
        (
            "git+ssh://git@github.com/o/r",
            repository("github.com", "o", "r"),
        ),
        // https, with and without userinfo, which is never kept.
        (
            "https://github.com/o/r.git",
            repository("github.com", "o", "r"),
        ),
        (
            "https://github.com/o/r/",
            repository("github.com", "o", "r"),
        ),
        ("http://github.com/o/r", repository("github.com", "o", "r")),
        (
            "https://user:ghp_token@github.com/o/r.git",
            repository("github.com", "o", "r"),
        ),
        // GHE.com and Enterprise Server.
        (
            "https://acme.ghe.com/team/app",
            repository("acme.ghe.com", "team", "app"),
        ),
        (
            "git@ghe.example.com:team/app.git",
            repository("ghe.example.com", "team", "app"),
        ),
        (
            "  https://github.com/o/r.js.git\n",
            repository("github.com", "o", "r.js"),
        ),
    ];
    for (url, expected) in cases {
        assert_eq!(parse(url), expected, "{url}");
    }
}

#[test]
fn what_is_not_a_hosted_repository_is_refused() {
    for url in [
        "",
        "/srv/git/project.git",
        "../project",
        "file:///srv/git/o/r.git",
        "C:\\code\\project",
        "github.com/o/r",
        "https://github.com/o",
        "https://github.com/o/r/tree/main",
        "https://github.com:8443/o/r",
        "https://github.com/../r",
        "https://github.com/o/r?x=1",
        "git@github.com:o/r b",
        "https://a@b@github.com/o/r",
        "ssh://git@github.com:x/o/r",
        // An SSH alias from ~/.ssh/config names no host.
        "work:o/r",
        "ftp://github.com/o/r",
    ] {
        assert_eq!(parse(url), None, "{url}");
    }
}
