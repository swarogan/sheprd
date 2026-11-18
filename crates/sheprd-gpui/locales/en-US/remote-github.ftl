# Lending this computer's GitHub sign-in to a paired phone, so the phone
# reads pull requests and checks from GitHub itself: a switch in the pairing
# dialog and in Settings > Remote control's device list. gh is the GitHub
# CLI's command name.

remote-github-share = Share GitHub sign-in with this phone
# $login is the GitHub account Sheprd is signed in with on github.com.
remote-github-share-account = The phone reads GitHub as { $login }, and with any account gh is signed in to on this computer.
remote-github-share-gh = The phone reads GitHub with the accounts gh is signed in to on this computer, if any.
remote-github-share-note = Turning it off stops new sign-ins. One from gh that the phone already has works until you run gh auth refresh or revoke it on GitHub.
# $error says why, in the daemon's words.
remote-github-share-failed = Could not change GitHub sharing: { $error }
