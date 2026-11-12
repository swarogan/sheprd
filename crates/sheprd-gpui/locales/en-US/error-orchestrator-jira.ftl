# Why a Jira Cloud site's issues could not be read, or its address, email, and
# API token could not be saved.
# [jira] allow_plaintext_credentials = true is a setting: keep it as it is.
# read:jira-work, read:jira-user, and write:jira-work are Atlassian scope names: keep them as they are.

# $address is what was typed, quoted.
error-orchestrator-jira-address = { $address } is not an https address of a Jira Cloud site, such as https://acme.atlassian.net
error-orchestrator-jira-authentication = Jira rejected the email or API token; replace them from the account menu with an Atlassian API token. A token with scopes needs the read:jira-work and read:jira-user scopes
error-orchestrator-jira-directory = The folder for saved Jira API tokens could not be found: { $error }
error-orchestrator-jira-email = The Atlassian account email must be an address such as you@example.com
error-orchestrator-jira-email-length = An Atlassian account email is at most { $limit } characters
error-orchestrator-jira-json = Jira's answer is not the expected JSON: { $error }
# $origin is the address tried, so a wrong one shows.
error-orchestrator-jira-network = Jira could not be reached at { $origin }: { $error }
error-orchestrator-jira-no-keyring = No desktop keyring is running to save a Jira API token in. Start GNOME Keyring, KWallet, or KeePassXC, or set [jira] allow_plaintext_credentials = true
error-orchestrator-jira-no-store = No secure credential store is available to save a Jira API token in. Set [jira] allow_plaintext_credentials = true
error-orchestrator-jira-not-found = { $origin } does not answer as a Jira Cloud site; check the address
error-orchestrator-jira-rate-limit = Jira rate limit reached; it is read again later
error-orchestrator-jira-read = Jira's answer could not be read: { $error }
error-orchestrator-jira-record = The saved Jira sites are not in the expected form
error-orchestrator-jira-record-size = The saved Jira sites would be too large to read back; forget a site or use a shorter token
error-orchestrator-jira-sign-in = Connect Jira from the account menu to read the issues assigned to you on { $site }
error-orchestrator-jira-size = Jira's answer exceeded the size limit
error-orchestrator-jira-status = Jira request failed (HTTP { $status })
error-orchestrator-jira-store = The Jira API token could not be read or saved: { $error }
error-orchestrator-jira-token = The Jira API token is not one an HTTP header can carry
error-orchestrator-jira-token-length = A Jira API token is at most { $limit } characters; paste the token Atlassian showed
error-orchestrator-jira-too-many-sites = Jira sites are saved for at most { $limit } sites
error-orchestrator-jira-comment = Write a comment before posting it
error-orchestrator-jira-comment-length = A comment is at most { $limit } bytes
error-orchestrator-jira-forbidden = Jira refused the change; a token with scopes needs the write:jira-work scope, and your account needs permission on the issue
error-orchestrator-jira-issue = That is not the key of a Jira issue
error-orchestrator-jira-transition = That is not the number of a Jira transition
