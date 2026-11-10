//! Gitea addresses and tokens pasted in the account menu: kept per hostname
//! in one record, read back by a remote's host, and never kept in a form the
//! store would refuse to read.
use crate::gitea::{
    self,
    saved::{
        MAX_INSTANCES, Saved, TOKEN_LIMIT, fits, hostname, normalize_address, read_in, save_in,
    },
};
use secrecy::{ExposeSecret, SecretString};
use sheprd_credentials::Store;

fn secret(text: &str) -> SecretString {
    SecretString::from(text)
}

#[test]
fn an_address_is_normalized_to_its_host_and_base_url() {
    for (typed, host, base) in [
        ("http://gitea.arpa", "gitea.arpa", "http://gitea.arpa"),
        (" HTTP://Gitea.Arpa/ ", "gitea.arpa", "http://gitea.arpa"),
        (
            "https://example.com/gitea/",
            "example.com",
            "https://example.com/gitea",
        ),
        (
            "http://gitea.arpa:3000",
            "gitea.arpa",
            "http://gitea.arpa:3000",
        ),
        ("gitea.arpa", "gitea.arpa", "https://gitea.arpa"),
        (
            "gitea.arpa:3000/sub/path//",
            "gitea.arpa",
            "https://gitea.arpa:3000/sub/path",
        ),
        ("https://gitea.arpa:443", "gitea.arpa", "https://gitea.arpa"),
    ] {
        assert_eq!(
            normalize_address(typed),
            Some((host.to_owned(), base.to_owned())),
            "{typed}"
        );
    }
    for refused in [
        "",
        "ftp://gitea.arpa",
        "ssh://git@gitea.arpa",
        "http://user@gitea.arpa",
        "http://user:pw@gitea.arpa",
        "http://gitea.arpa?x=1",
        "http://gitea.arpa/#top",
        "http://",
        &format!("http://gitea.arpa/{}", "a".repeat(512)),
    ] {
        assert_eq!(normalize_address(refused), None, "{refused}");
    }
}

#[test]
fn a_hostname_is_lowercase_without_a_port() {
    assert_eq!(hostname("Gitea.Arpa:3000"), "gitea.arpa");
    assert_eq!(hostname("gitea.arpa"), "gitea.arpa");
    assert_eq!(hostname("[::1]:3000"), "[::1]");
    assert_eq!(hostname("[::1]"), "[::1]");
}

#[test]
fn saved_instances_read_back_by_host_and_forget_one_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    assert_eq!(read_in(Store::File, path).unwrap().instances().count(), 0);
    save_in(
        Store::File,
        path,
        "http://Gitea.arpa:3000/",
        Some(secret(" tok-one \n")),
    )
    .unwrap();
    save_in(
        Store::File,
        path,
        "https://example.com/gitea",
        Some(secret("tok-two")),
    )
    .unwrap();
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(
        saved.instances().map(|(host, _)| host).collect::<Vec<_>>(),
        ["example.com", "gitea.arpa"]
    );
    // A remote's port and case do not matter; the typed address is kept.
    let kept = saved.get("GITEA.arpa:22").unwrap();
    assert_eq!(kept.base_url(), "http://gitea.arpa:3000");
    assert_eq!(kept.token().expose_secret(), "tok-one");
    assert_eq!(
        saved.get("example.com").unwrap().base_url(),
        "https://example.com/gitea"
    );
    assert!(saved.get("other.arpa").is_none());

    // A new address for a host replaces the old one; a bare host forgets it.
    save_in(
        Store::File,
        path,
        "https://gitea.arpa",
        Some(secret("tok-new")),
    )
    .unwrap();
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(
        saved.get("gitea.arpa").unwrap().base_url(),
        "https://gitea.arpa"
    );
    assert_eq!(saved.instances().count(), 2);
    save_in(Store::File, path, "example.com", None).unwrap();
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(
        saved.instances().map(|(host, _)| host).collect::<Vec<_>>(),
        ["gitea.arpa"]
    );
    save_in(Store::File, path, "gitea.arpa", None).unwrap();
    // The last forget removes the file rather than leaving an empty record.
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn a_bad_address_or_token_is_refused_before_it_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    assert!(matches!(
        save_in(Store::File, path, "ftp://gitea.arpa", Some(secret("tok"))),
        Err(gitea::Error::Address(_))
    ));
    assert!(matches!(
        save_in(Store::File, path, "gitea.arpa", Some(secret("two words"))),
        Err(gitea::Error::Token)
    ));
    assert!(matches!(
        save_in(Store::File, path, "gitea.arpa", Some(secret(""))),
        Err(gitea::Error::Token)
    ));
    let long = "p".repeat(TOKEN_LIMIT + 1);
    assert!(matches!(
        save_in(Store::File, path, "gitea.arpa", Some(secret(&long))),
        Err(gitea::Error::TokenLength { limit: TOKEN_LIMIT })
    ));
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn the_record_holds_a_bounded_number_of_instances() {
    let mut saved = Saved::default();
    for index in 0..MAX_INSTANCES {
        saved
            .set(&format!("http://host{index}.arpa"), Some(secret("tok")))
            .unwrap();
    }
    assert!(matches!(
        saved.set("http://one-more.arpa", Some(secret("tok"))),
        Err(gitea::Error::TooManyInstances)
    ));
    // Replacing a kept instance's address is not one more.
    saved
        .set("https://host0.arpa", Some(secret("tok-new")))
        .unwrap();
    let record: String = (0..=MAX_INSTANCES)
        .map(|index| format!("host{index}.arpa\thttp://host{index}.arpa\ttok\n"))
        .collect();
    assert!(matches!(
        Saved::parse(&secret(&record)),
        Err(gitea::Error::Record)
    ));
}

#[test]
fn a_malformed_record_is_refused_unread_and_replaced_by_the_next_save() {
    for record in [
        "gitea.arpa",
        "gitea.arpa\thttp://gitea.arpa",
        "gitea.arpa\thttp://gitea.arpa\ttok\textra\n",
        // The host must be the address's own, lowercase, and the address normal.
        "other.arpa\thttp://gitea.arpa\ttok\n",
        "Gitea.arpa\thttp://gitea.arpa\ttok\n",
        "gitea.arpa\thttp://gitea.arpa/\ttok\n",
        "gitea.arpa\tftp://gitea.arpa\ttok\n",
        "gitea.arpa\thttp://gitea.arpa\ttwo words\n",
        "gitea.arpa\thttp://gitea.arpa\t\n",
    ] {
        assert!(
            matches!(Saved::parse(&secret(record)), Err(gitea::Error::Record)),
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
            .open(path.join("gitea-credentials"))
            .unwrap();
        std::io::Write::write_all(&mut file, b"not a record\n").unwrap();
    }
    assert!(matches!(
        read_in(Store::File, path),
        Err(gitea::Error::Record)
    ));
    // Recognizing a remote skips an unreadable record rather than failing.
    assert_eq!(
        gitea::saved::load(Store::Environment).instances().count(),
        0
    );
    save_in(
        Store::File,
        path,
        "http://gitea.arpa",
        Some(secret("tok-new")),
    )
    .unwrap();
    assert_eq!(
        read_in(Store::File, path)
            .unwrap()
            .get("gitea.arpa")
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
    let name = |index: usize| format!("{index:02}{}.example.com", "h".repeat(60));
    for index in 0..MAX_INSTANCES {
        let address = format!("https://{}/{}", name(index), "s".repeat(100));
        save_in(Store::File, path, &address, Some(secret(&token))).unwrap();
    }
    let saved = read_in(Store::File, path).unwrap();
    assert_eq!(saved.instances().count(), MAX_INSTANCES);
    assert_eq!(
        saved
            .get(&name(MAX_INSTANCES - 1))
            .unwrap()
            .token()
            .expose_secret(),
        token
    );
    // Forget still works on the fullest record.
    save_in(Store::File, path, &name(0), None).unwrap();
    assert_eq!(
        read_in(Store::File, path).unwrap().instances().count(),
        MAX_INSTANCES - 1
    );
    // A record past the store's limit is never written over one that reads.
    let oversized = secret(&"x".repeat(sheprd_credentials::LIMIT + 1));
    assert!(matches!(fits(&oversized), Err(gitea::Error::RecordSize)));
    assert!(fits(&secret("gitea.arpa\thttp://gitea.arpa\ttok\n")).is_ok());
}

#[test]
fn no_credential_store_says_how_to_opt_in_for_gitea() {
    let error = gitea::Error::from(sheprd_credentials::Error::Policy);
    assert!(matches!(error, gitea::Error::NoStore(_)));
    assert!(
        error
            .to_string()
            .contains("[gitea] allow_plaintext_credentials")
    );
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(gitea::Config::default().store(), Store::for_policy(false));
    let opted = gitea::Config {
        allow_plaintext_credentials: true,
    };
    assert_eq!(opted.store(), Store::for_policy(true));
}

#[test]
fn only_a_missing_or_refused_token_offers_the_sign_in() {
    let repository = || "ops/infra".to_owned();
    assert!(
        gitea::Error::SignIn {
            repository: repository()
        }
        .needs_sign_in()
    );
    assert!(gitea::Error::Authentication.needs_sign_in());
    assert!(
        !gitea::Error::NotFound {
            repository: repository()
        }
        .needs_sign_in()
    );
    assert!(!gitea::Error::RateLimit.needs_sign_in());
}
