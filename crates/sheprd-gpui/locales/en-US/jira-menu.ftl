# The account menu's Jira page: each Jira Cloud site a sign-in is saved for,
# whom it signs in as, and typing a new address, email, and API token. $host is
# a site's hostname, such as acme.atlassian.net; $name the name Jira shows for
# an account, $email its Atlassian email, $user the name of the account a save
# signed in as. https and id.atlassian.com are an address scheme and a website:
# keep them as they are.

jira-menu-help = Type your Jira Cloud site's address, your Atlassian account email, and an API token to list the issues assigned to you that are not done. Sites must use https.
jira-menu-no-store = This window does not keep Jira sign-ins.
jira-menu-address = Site address
jira-menu-email = Email
jira-menu-token = API token
jira-menu-create-token = Create an API token at id.atlassian.com
jira-menu-check-and-save = Check and save
jira-menu-forget = Forget
jira-menu-address-first = Type the site's address first.
jira-menu-email-first = Type your Atlassian email first.
jira-menu-paste-first = Paste an API token first.

jira-menu-checking = Checking the sign-in…
jira-menu-signed-in = Signed in as { $name } ({ $email }).
jira-menu-refused = Jira refused the saved email or API token. Enter them again.
jira-menu-record-failed = { $error } Saving a sign-in replaces them.
jira-menu-saved = Saved for { $host }, signed in as { $user }.
jira-menu-removed = Forgot the sign-in for { $host }.

# The Issues section's banner while a Jira site waits for a sign-in; clicking
# it opens this page. $site is the site's hostname.
jira-menu-connect-banner = Connect Jira to read the issues assigned to you on { $site }
