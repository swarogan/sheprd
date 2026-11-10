//! Jira sites and sign-ins pasted in the account menu: kept per hostname in
//! one record, read back by a site's host, and never kept in a form the
//! store would refuse to read.
use crate::jira::{
    self,
    saved::{
        EMAIL_LIMIT, Login, MAX_SITES, Saved, TOKEN_LIMIT, checked_email, fits, hostname,
        normalize_address, read_in, save_in,
    },
};
use secrecy::{ExposeSecret, SecretString};
use sheprd_credentials::Store;

fn secret(text: &str) -> SecretString {
    SecretString::from(text)
}

fn login(email: &str, token: &str) -> Option<Login> {
    Some(Login {
        email: email.to_owned(),
        token: secret(token),
    })
}

#[test]
fn an_address_is_normalized_to_its_host_and_https_base_url() {
    for (typed, host, base) in [
        (
            "https://acme.atlassian.net",
            "acme.atlassian.net",
            "https://acme.atlassian.net",
        ),
        (
            " HTTPS://Acme.Atlassian.net/ ",
            "acme.atlassian.net",
            "https://acme.atlassian.net",
        ),
        (
            "acme.atlassian.net",
            "acme.atlassian.net",
            "https://acme.atlassian.net",
        ),
        (
            "acme.atlassian.net/",
            "acme.atlassian.net",
            "https://acme.atlassian.net",
        ),
        (
            "https://example.com/jira/",
            "example.com",
            "https://example.com/jira",
        ),
        (
            "jira.example.com:8443/sub//",
            "jira.example.com",
            "https://jira.example.com:8443/sub",
        ),
        (
            "https://acme.atlassian.net:443",
            "acme.atlassian.net",
            "https://acme.atlassian.net",
        ),
    ] {
        assert_eq!(
            normalize_address(typed),
            Some((host.to_owned(), base.to_owned())),
            "{typed}"
        );
    }
    for refused in [
        "",
        "http://acme.atlassian.net",
        "HTTP://acme.atlassian.net",
        "ftp://acme.atlassian.net",
        "ssh://git@acme.atlassian.net",
        "https://me@acme.atlassian.net",
        "https://me:pw@acme.atlassian.net",
        "https://acme.atlassian.net?x=1",
        "https://acme.atlassian.net/#top",
        "https://",
        &format!("https://acme.atlassian.net/{}", "a".repeat(512)),
    ] {
        assert_eq!(normalize_address(refused), None, "{refused}");
    }
}

#[test]
fn an_email_is_one_address_without_spaces_or_the_colon_that_splits_basic_credentials() {
    assert_eq!(
        checked_email("  me@example.com \n").as_deref().ok(),
        Some("me@example.com")
    );
    for bad in [
        "",
        "me",
        "@example.com",
        "me@",
        "a@b@c",
        "me @example.com",
        "m:e@example.com",
        "me@exa\tmple.com",
    ] {
        assert!(
            matches!(checked_email(bad), Err(jira::Error::Email)),
            "{bad:?}"
        );
    }
    let long = format!("{}@example.com", "a".repeat(EMAIL_LIMIT));
    assert!(matches!(
        checked_email(&long),
        Err(jira::Error::EmailLength { limit: EMAIL_LIMIT })
    ));
}

#[test]
fn a_hostname_is_lowercase_without_a_port() {
    assert_eq!(hostname("Acme.Atlassian.net:443"), "acme.atlassian.net");
    assert_eq!(hostname("acme.atlassian.net"), "acme.atlassian.net");
}

#[test]
fn saved_sites_read_back_by_host_and_forget_one_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    assert_eq!(read_in(Store::File, path).unwrap().sites().count(), 0);
    save_in(
        Store::File,
        path,
        "Acme.Atlassian.net/",
        login(" me@example.com ", " tok-one \n"),
    )
    .unwrap();
    save_in(
        Store::File,
        path,
        "https://example.com/jira",
        login("you@example.com", "tok-two"),
    )
    .unwrap();
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(
        saved.sites().map(|(host, _)| host).collect::<Vec<_>>(),
        ["acme.atlassian.net", "example.com"]
    );
    let kept = saved.get("ACME.atlassian.net:22").unwrap();
    assert_eq!(kept.base_url(), "https://acme.atlassian.net");
    assert_eq!(kept.email(), "me@example.com");
    assert_eq!(kept.token().expose_secret(), "tok-one");
    assert_eq!(
        saved.get("example.com").unwrap().base_url(),
        "https://example.com/jira"
    );
    assert!(saved.get("other.example").is_none());

    // The same host again replaces the site; a bare host without a login forgets it.
    save_in(
        Store::File,
        path,
        "acme.atlassian.net",
        login("new@example.com", "tok-new"),
    )
    .unwrap();
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(
        saved.get("acme.atlassian.net").unwrap().email(),
        "new@example.com"
    );
    assert_eq!(saved.sites().count(), 2);
    save_in(Store::File, path, "example.com", None).unwrap();
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(
        saved.sites().map(|(host, _)| host).collect::<Vec<_>>(),
        ["acme.atlassian.net"]
    );
    save_in(Store::File, path, "acme.atlassian.net", None).unwrap();
    // The last forget removes the file rather than leaving an empty record.
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn a_bad_address_email_or_token_is_refused_before_it_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let save = |address: &str, email: &str, token: &str| {
        save_in(Store::File, path, address, login(email, token))
    };
    assert!(matches!(
        save("http://acme.atlassian.net", "me@example.com", "tok"),
        Err(jira::Error::Address(_))
    ));
    assert!(matches!(
        save("ftp://acme.atlassian.net", "me@example.com", "tok"),
        Err(jira::Error::Address(_))
    ));
    assert!(matches!(
        save("acme.atlassian.net", "", "tok"),
        Err(jira::Error::Email)
    ));
    assert!(matches!(
        save("acme.atlassian.net", "not-an-email", "tok"),
        Err(jira::Error::Email)
    ));
    assert!(matches!(
        save("acme.atlassian.net", "me@example.com", "two words"),
        Err(jira::Error::Token)
    ));
    assert!(matches!(
        save("acme.atlassian.net", "me@example.com", ""),
        Err(jira::Error::Token)
    ));
    let long = "p".repeat(TOKEN_LIMIT + 1);
    assert!(matches!(
        save("acme.atlassian.net", "me@example.com", &long),
        Err(jira::Error::TokenLength { limit: TOKEN_LIMIT })
    ));
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn the_record_holds_a_bounded_number_of_sites() {
    let mut saved = Saved::default();
    for index in 0..MAX_SITES {
        saved
            .set(
                &format!("host{index}.example.com"),
                login("me@example.com", "tok"),
            )
            .unwrap();
    }
    assert!(matches!(
        saved.set("one-more.example.com", login("me@example.com", "tok")),
        Err(jira::Error::TooManySites)
    ));
    // Replacing a kept site is not one more, and forgetting still works.
    saved
        .set("host0.example.com", login("me@example.com", "tok-new"))
        .unwrap();
    saved.set("host1.example.com", None).unwrap();
    let record: String = (0..=MAX_SITES)
        .map(|n| format!("host{n}.example.com\thttps://host{n}.example.com\tme@example.com\ttok\n"))
        .collect();
    assert!(matches!(
        Saved::parse(&secret(&record)),
        Err(jira::Error::Record)
    ));
}

#[test]
fn a_malformed_record_is_refused_unread_and_replaced_by_the_next_save() {
    for record in [
        "acme.example",
        "acme.example\thttps://acme.example\tme@example.com",
        "acme.example\thttps://acme.example\ttok\n",
        "acme.example\thttps://acme.example\tme@example.com\ttok\textra\n",
        // The host must be the address's own, lowercase, and the address normal and https.
        "other.example\thttps://acme.example\tme@example.com\ttok\n",
        "Acme.example\thttps://acme.example\tme@example.com\ttok\n",
        "acme.example\thttps://acme.example/\tme@example.com\ttok\n",
        "acme.example\thttp://acme.example\tme@example.com\ttok\n",
        "acme.example\thttps://acme.example\tnot-an-email\ttok\n",
        "acme.example\thttps://acme.example\t me@example.com\ttok\n",
        "acme.example\thttps://acme.example\tme@example.com\ttwo words\n",
        "acme.example\thttps://acme.example\tme@example.com\t\n",
    ] {
        assert!(
            matches!(Saved::parse(&secret(record)), Err(jira::Error::Record)),
            "{record:?}"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path.join("jira-credentials"))
            .unwrap();
        std::io::Write::write_all(&mut file, b"not a record\n").unwrap();
    }
    assert!(matches!(
        read_in(Store::File, path),
        Err(jira::Error::Record)
    ));
    // Listing sources skips an unreadable record rather than failing.
    assert_eq!(jira::saved::load(Store::Environment).sites().count(), 0);
    save_in(
        Store::File,
        path,
        "acme.example",
        login("me@example.com", "tok-new"),
    )
    .unwrap();
    assert_eq!(
        read_in(Store::File, path)
            .unwrap()
            .get("acme.example")
            .unwrap()
            .token()
            .expose_secret(),
        "tok-new"
    );
}

#[test]
fn the_fullest_record_the_store_accepts_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let token = "p".repeat(TOKEN_LIMIT);
    let email = format!(
        "{}@example.com",
        "e".repeat(EMAIL_LIMIT - "@example.com".len())
    );
    let name = |index: usize| format!("{index:02}{}.example.com", "h".repeat(60));
    for index in 0..MAX_SITES {
        let address = format!("https://{}/{}", name(index), "s".repeat(100));
        save_in(Store::File, path, &address, login(&email, &token)).unwrap();
    }
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(saved.sites().count(), MAX_SITES);
    let last = saved.get(&name(MAX_SITES - 1)).unwrap();
    assert_eq!(
        (last.email(), last.token().expose_secret()),
        (email.as_str(), token.as_str())
    );
    save_in(Store::File, path, &name(0), None).unwrap();
    assert_eq!(
        read_in(Store::File, path).unwrap().sites().count(),
        MAX_SITES - 1
    );
    // A record past the store's limit is never written over one that reads.
    let oversized = secret(&"x".repeat(sheprd_credentials::LIMIT + 1));
    assert!(matches!(fits(&oversized), Err(jira::Error::RecordSize)));
    assert!(
        fits(&secret(
            "acme.example\thttps://acme.example\tme@example.com\ttok\n"
        ))
        .is_ok()
    );
}

#[test]
fn no_credential_store_says_how_to_opt_in_for_jira() {
    let error = jira::Error::from(sheprd_credentials::Error::Policy);
    assert!(matches!(error, jira::Error::NoStore(_)));
    assert!(
        error
            .to_string()
            .contains("[jira] allow_plaintext_credentials")
    );
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(jira::Config::default().store(), Store::for_policy(false));
    let opted = jira::Config {
        allow_plaintext_credentials: true,
    };
    assert_eq!(opted.store(), Store::for_policy(true));
}
