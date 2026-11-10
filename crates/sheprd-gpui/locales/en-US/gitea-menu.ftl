# The account menu's Gitea page: each instance a token is saved for, whom it
# signs in as, and pasting a new address and token. $host is an instance's
# hostname, such as gitea.example.com; $user the name Gitea shows for an
# account, $login its sign-in name. read:repository, read:issue, and read:user
# are Gitea token scopes, and http and https address schemes: keep them as they are.

gitea-menu-help = Type the address of your Gitea or Forgejo instance and paste an access token to read its issues and pull requests. Create the token under Settings > Applications on the instance, with the read:repository, read:issue, and read:user scopes.
gitea-menu-no-store = This window does not keep Gitea tokens.
gitea-menu-address = Instance address
gitea-menu-token = Access token
gitea-menu-check-and-save = Check and save
gitea-menu-forget = Forget
gitea-menu-address-first = Type the instance's address first.
gitea-menu-paste-first = Paste a token first.
gitea-menu-plain-http = This address uses http, so the token travels unencrypted.

gitea-menu-checking = Checking the token…
gitea-menu-signed-in = Signed in as { $login }.
gitea-menu-refused = Gitea refused the saved token. Paste a new one.
gitea-menu-record-failed = { $error } Saving a token replaces them.
gitea-menu-saved = Saved for { $host }, signed in as { $user }.
gitea-menu-removed = Forgot the token for { $host }.

# The Pull requests section's banner while a Gitea repository waits for a
# token; clicking it opens this page.
gitea-menu-connect-banner = Connect Gitea to read { $repository }
