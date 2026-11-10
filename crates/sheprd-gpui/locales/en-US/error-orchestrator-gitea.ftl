# Why a Gitea (or Forgejo) instance's issues or pull requests could not be read,
# or its address and token could not be saved.
# [gitea] allow_plaintext_credentials = true is a setting: keep it as it is.

# $address is what was typed, quoted.
error-orchestrator-gitea-address = { $address } is not an address of a Gitea instance, such as http://gitea.example.com
error-orchestrator-gitea-authentication = Gitea rejected the token; replace it from the account menu with one that can read repositories and issues
error-orchestrator-gitea-directory = The folder for saved Gitea tokens could not be found: { $error }
error-orchestrator-gitea-json = Gitea's answer is not the expected JSON: { $error }
# $origin is the address tried, so a wrong one shows.
error-orchestrator-gitea-network = Gitea could not be reached at { $origin }: { $error }
error-orchestrator-gitea-no-keyring = No desktop keyring is running to save a Gitea token in. Start GNOME Keyring, KWallet, or KeePassXC, or set [gitea] allow_plaintext_credentials = true
error-orchestrator-gitea-no-store = No secure credential store is available to save a Gitea token in. Set [gitea] allow_plaintext_credentials = true
error-orchestrator-gitea-not-found = Gitea has no repository { $repository }, or the token cannot see it
error-orchestrator-gitea-not-gitea = { $origin } does not answer as a Gitea instance; check the address and any sub-path
error-orchestrator-gitea-rate-limit = Gitea rate limit reached; it is read again later
error-orchestrator-gitea-read = Gitea's answer could not be read: { $error }
error-orchestrator-gitea-record = The saved Gitea tokens are not in the expected form
error-orchestrator-gitea-record-size = The saved Gitea tokens would be too large to read back; forget an instance or use a shorter token
error-orchestrator-gitea-sign-in = Connect Gitea from the account menu to read { $repository }
error-orchestrator-gitea-size = Gitea's answer exceeded the size limit
error-orchestrator-gitea-status = Gitea request failed (HTTP { $status })
error-orchestrator-gitea-store = The Gitea token could not be read or saved: { $error }
error-orchestrator-gitea-token = The Gitea token is not one an HTTP header can carry
error-orchestrator-gitea-token-length = A Gitea access token is at most { $limit } characters; paste the token Gitea showed
error-orchestrator-gitea-too-many-instances = Gitea tokens are saved for at most { $limit } instances
