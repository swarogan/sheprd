//! The `en-XA` leak test (`crate::i18n::leak_tests`) for the account menu's
//! Gitea page: its cards and form draw only translated messages, except for
//! addresses, account names, and the fields' placeholders.
use super::*;
use crate::i18n::leak_tests::{assert_translated, under_en_xa};
use sheprd_orchestrator::gitea::account::Account;

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
fn the_gitea_page_shows_only_messages_under_en_xa() {
    under_en_xa("Gitea page en-XA leak", |cx, recorder| {
        let (view, cx) = page(cx);
        recorder.take();
        view.update_in(cx, |view, _, cx| {
            let account = Account {
                base_url: "http://gitea.arpa:3000".into(),
                host: "gitea.arpa".into(),
                login: "alice".into(),
                name: "Alice Example".into(),
            };
            view.menu.gitea.instances = vec![
                status(Lookup::Signed(account)),
                InstanceStatus {
                    host: "forge.example.com".into(),
                    address: "https://forge.example.com".into(),
                    lookup: Lookup::Unknown,
                },
            ];
            // An http address brings the unencrypted-token note.
            if let Some(input) = view.menu.gitea.address.clone() {
                input.update(cx, |input, cx| input.set_text_selected("http://lan", cx));
            }
            cx.notify();
        });
        draw(cx);
        let data = cut(&[
            "GitHub",
            "GitLab",
            "Gitea",
            "Azure DevOps",
            "Jira",
            "gitea.arpa",
            "http://gitea.arpa:3000",
            "forge.example.com",
            "https://forge.example.com",
            "http://lan",
            "Alice Example",
            "https://gitea.example.com",
            "\u{2022}\u{2022}\u{2022}\u{2022}",
        ]);
        assert_translated("Gitea > instances", &recorder.take(), &data);
    });
}
