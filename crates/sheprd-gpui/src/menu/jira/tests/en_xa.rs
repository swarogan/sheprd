//! The `en-XA` leak test (`crate::i18n::leak_tests`) for the account menu's
//! Jira page: its cards and form draw only translated messages, except for
//! addresses, account names, and the fields' placeholders.
use super::*;
use crate::i18n::leak_tests::{assert_translated, under_en_xa};
use sheprd_orchestrator::jira::account::Account;

/// `names` as a narrow row may show them: whole, cut short at any point
/// and ended with "…", and the single letters shaped to measure the cut.
fn cut(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .flat_map(|name| {
            let cuts = name
                .char_indices()
                .skip(1)
                .map(|(at, _)| format!("{}\u{2026}", name[..at].trim_end()))
                .collect::<Vec<_>>();
            let letters = name.chars().map(String::from).collect::<Vec<_>>();
            std::iter::once((*name).to_owned())
                .chain(cuts)
                .chain(letters)
        })
        .collect()
}

#[test]
fn the_jira_page_shows_only_messages_under_en_xa() {
    under_en_xa("Jira page en-XA leak", |cx, recorder| {
        let (view, cx) = page(cx);
        recorder.take();
        view.update_in(cx, |view, _, cx| {
            let account = Account {
                account_id: "5b10ac8d".into(),
                base_url: "https://acme.atlassian.net".into(),
                email: Some("ada@work.example".into()),
                host: "acme.atlassian.net".into(),
                name: "Ada Lovelace".into(),
            };
            view.menu.jira.sites = vec![
                status(Lookup::Signed(account)),
                SiteStatus {
                    host: "forge.example.com".into(),
                    address: "https://forge.example.com".into(),
                    email: "me@example.com".into(),
                    lookup: Lookup::Unknown,
                },
            ];
            cx.notify();
        });
        draw(cx);
        let data = cut(&[
            "GitHub",
            "GitLab",
            "Gitea",
            "Azure DevOps",
            "Jira",
            "acme.atlassian.net",
            "https://acme.atlassian.net",
            "forge.example.com",
            "https://forge.example.com",
            "Ada Lovelace",
            "ada@work.example",
            "example.atlassian.net",
            "you@example.com",
            "\u{2022}\u{2022}\u{2022}\u{2022}",
        ]);
        assert_translated("Jira > sites", &recorder.take(), &data);
    });
}
