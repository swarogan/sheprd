use super::*;
use crate::control::changes::AskId;
use crate::control::github::{Lease, LeaseAnswer, LeaseAsk, Token, valid_host, valid_login};

fn lease() -> Lease {
    Lease {
        login: "octocat".into(),
        token: Token::new("ghu_secret".into()),
        expires_at: Some(1_800_000_000),
    }
}

#[test]
fn an_approval_says_so_only_when_github_is_not_shared() {
    let approve = |share_github| Request::Approve {
        id: PairingId("p1".into()),
        allow: true,
        code: "428913".into(),
        hosts: Vec::new(),
        share_github,
    };
    assert_eq!(
        serde_json::to_value(approve(false)).unwrap(),
        json!({"request": "approve", "id": "p1", "allow": true, "code": "428913", "share_github": false})
    );
    // An older Sheprd's approval shares, as a new pairing does by default.
    let older: Request = serde_json::from_value(
        json!({"request": "approve", "id": "p1", "allow": true, "code": "428913"}),
    )
    .unwrap();
    assert_eq!(older, approve(true));
}

#[test]
fn lease_requests_keep_their_wire_names() {
    assert_eq!(
        serde_json::to_value(Request::GitHubLeaseAsks {
            login: Some("octocat".into())
        })
        .unwrap(),
        json!({"request": "github_lease_asks", "login": "octocat"})
    );
    assert_eq!(
        serde_json::to_value(Request::GitHubLease {
            id: AskId(3),
            answer: LeaseAnswer::Lease(lease()),
        })
        .unwrap(),
        json!({
            "request": "github_lease",
            "id": 3,
            "answer": {"answer": "lease", "body": {
                "login": "octocat", "token": "ghu_secret", "expires_at": 1_800_000_000u64
            }},
        })
    );
    assert_eq!(
        serde_json::to_value(Request::ShareGitHub {
            device: DeviceId("d".into()),
            share: false,
        })
        .unwrap(),
        json!({"request": "share_github", "device": "d", "share": false})
    );
    let response = Response::GitHubLeaseAsks {
        asks: vec![LeaseAsk {
            id: AskId(1),
            host: "github.com".into(),
        }],
    };
    let wire = serde_json::to_value(&response).unwrap();
    assert_eq!(
        wire,
        json!({"response": "github_lease_asks", "asks": [{"id": 1, "host": "github.com"}]})
    );
    assert_eq!(serde_json::from_value::<Response>(wire).unwrap(), response);
    // An answer this Sheprd does not know reads as signed out.
    let unknown: LeaseAnswer = serde_json::from_value(json!({"answer": "later"})).unwrap();
    assert_eq!(unknown, LeaseAnswer::SignedOut);
}

#[test]
fn a_token_never_shows_in_debug() {
    let shown = [
        format!("{:?}", lease()),
        format!("{:?}", LeaseAnswer::Lease(lease())),
        format!(
            "{:?}",
            Request::GitHubLease {
                id: AskId(1),
                answer: LeaseAnswer::Lease(lease()),
            }
        ),
    ];
    for text in shown {
        assert!(!text.contains("ghu_secret"), "{text}");
        assert!(text.contains("REDACTED"), "{text}");
    }
}

#[test]
fn a_lease_stays_within_bounds() {
    assert!(lease().valid());
    let bad = [
        Lease {
            login: String::new(),
            ..lease()
        },
        Lease {
            login: "octo cat".into(),
            ..lease()
        },
        Lease {
            token: Token::new("".into()),
            ..lease()
        },
        Lease {
            token: Token::new("a b".into()),
            ..lease()
        },
        Lease {
            token: Token::new("x".repeat(4097).into()),
            ..lease()
        },
    ];
    for lease in bad {
        assert!(!lease.valid(), "{lease:?}");
    }
    assert!(valid_login("octo-cat_1"));
    assert!(!valid_login(&"a".repeat(101)));
}

#[test]
fn only_plain_host_names_are_hosts() {
    for good in ["github.com", "ghe.example.com", "acme.ghe.com", "a-b.c1.io"] {
        assert!(valid_host(good), "{good}");
    }
    for bad in [
        "",
        "github",
        "GitHub.com",
        "github.com:443",
        "https://github.com",
        "user@github.com",
        "-a.com",
        "a-.com",
        "a..com",
        "github.com/",
    ] {
        assert!(!valid_host(bad), "{bad}");
    }
    assert!(!valid_host(&format!("{}.com", "a".repeat(64))));
}

#[test]
fn a_status_without_asks_reads_as_none_waiting() {
    let status: Status = serde_json::from_value(json!({
        "version": "1", "port": 7443, "addresses": [], "fingerprint": "F",
        "herdr": true, "chats": 0, "devices": 0, "connected": 0,
    }))
    .unwrap();
    assert_eq!(status.github_asks, 0);
}
