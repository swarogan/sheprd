# Settings > Remote control: the daemon's switch and how it serves, pairing
# a phone, and the paired devices. Also the pane history switch Settings
# saves on each SSH host.

settings-remote-health-updating = Updating the service…
settings-remote-health-off = Off. Turning it on installs a user service that serves your chats over HTTPS on your network, and over Tailscale when it runs.
settings-remote-health-starting = Starting…
settings-remote-health-running = Running on port { $port } · { $devices ->
        [one] { $devices } device
       *[other] { $devices } devices
    }, { $connected } connected
settings-remote-health-running-alone = Running on port { $port } · { $devices ->
        [one] { $devices } device
       *[other] { $devices } devices
    }, { $connected } connected · Herdr is not running
settings-remote-health-not-running = Not running: { $reason }
settings-remote-service-failed = Could not change the service: { $error }
settings-remote-revoke-failed = Could not revoke the device: { $error }
settings-remote-retry = Retry
# Shown while the service runs a build without a developer ID and the
# macOS firewall is on.
settings-remote-firewall = Phones will be blocked after this build changes
settings-remote-firewall-note = The macOS firewall is on, and this build of the daemon is not signed with a developer ID, so macOS approves only this exact build. After a rebuild, the next restart of the Mac or of the daemon blocks phones until someone clicks Allow at this computer. Run remote control from a signed release to keep the approval.

settings-remote-section-service = Service
settings-remote-keep-running-note = Phones keep working and notifications keep coming. Pairing a new device still needs Sheprd.
settings-remote-presence = Hold phone alerts while I'm here
settings-remote-presence-note = Paired iPhones near this computer over Bluetooth keep their alerts until you step away from it.
settings-remote-restart = Restart the daemon
settings-remote-restart-note = Starts it again from the binary beside Sheprd, as after an update. Phones reconnect on their own.
settings-remote-restart-button = Restart

settings-remote-section-connection = Connection
settings-remote-bind-all = All interfaces
settings-remote-addresses-none = No network address found.
settings-remote-while-running = Shown while remote control runs.
settings-remote-certificate-own = Own CA
settings-remote-certificate-files = My files
settings-remote-certificate-files-note = Your certificate and key, such as for your own domain.
settings-remote-certificate-own-note = The daemon's own certificate authority, trusted once on the phone. Its fingerprint shows while it runs.
settings-remote-certificate-fingerprint = The daemon's own certificate authority, trusted once on the phone. CA SHA-256 { $fingerprint }
settings-remote-files = Certificate files
settings-remote-files-certificate = Certificate
settings-remote-files-key = Private key
settings-remote-files-invalid = Give both files as absolute paths.
settings-remote-files-apply = Use these files

settings-remote-section-relays = Relays
settings-remote-relays = Reach this computer from any network
settings-remote-relays-none = No relay. Phones reach this computer on your network, or over Tailscale when it runs.
settings-remote-relays-note = Paired phones go through a relay when they have no direct path. A relay sees only encrypted traffic. A phone learns of a new relay the next time it connects directly.
settings-remote-relay-listening = Listening
settings-remote-relay-listening-rtt = Listening · { $ms } ms
settings-remote-relay-connecting = Connecting…
settings-remote-relay-failed = Failed: { $error }
settings-remote-relay-unreported = The daemon has not reported on it yet.
# $tags is the relay's tags from remote.relays, joined with commas.
settings-remote-relay-tags = Tags: { $tags }
settings-remote-relay-remove = Remove
settings-remote-relay-remove-label = Remove { $relay }
settings-remote-relay-add = Add a relay
settings-remote-relay-add-note = The address of a relay you run. Adding or removing one restarts the daemon.
settings-remote-relay-add-button = Add
settings-remote-public-relays = Sheprd's relays
settings-remote-public-relays-loading = Loading the list…
settings-remote-public-relays-none = No Sheprd relays are available for now. Run your own to reach this computer from anywhere.
settings-remote-public-relays-setup = How to run one
settings-remote-public-relays-validated = Validated by { $signer }. Add one to listen on it.
settings-remote-public-relay-named = { $name } ({ $region })
settings-remote-public-relay-added = Added
settings-remote-public-relay-add-label = Add { $relay }
settings-remote-relay-invalid = That is not a relay's address. Give one such as { $example }.

settings-remote-section-notifications = Notifications
settings-remote-notifications-note = Generic says which chat needs you without its text; Excerpt adds a short excerpt of the message.
settings-remote-privacy-generic = Generic
settings-remote-privacy-excerpt = Excerpt
settings-remote-live-activities = Live Activities
settings-remote-live-activities-note = Detailed shows the chat and its current step on a paired iPhone's lock screen; questions and their answer buttons also need Excerpt. Generic shows only the agent and what it is doing in general. A phone can ask for less in its own settings.
settings-remote-live-activities-detailed = Detailed
settings-remote-live-activities-generic = Generic

settings-remote-section-devices = Devices
settings-remote-idle-note = A device that has not connected for this long is forgotten and must pair again.
settings-remote-idle-day = 1 day
settings-remote-idle-week = 1 week
settings-remote-idle-month = 1 month
settings-remote-idle-never = Never
settings-remote-devices-none = No device is paired yet.
settings-remote-devices-count = { $count ->
        [one] { $count } device can control your chats.
       *[other] { $count } devices can control your chats.
    }
settings-remote-device-revoke-ask = Revoke it? Its sessions end and its notifications stop at once, here and on the machines it was paired with from here.
settings-remote-device-cancel = Cancel
settings-remote-device-revoke = Revoke
settings-remote-device-revoke-open = Revoke…
settings-remote-device-revoke-confirm = Confirm revoke

settings-remote-pair-off = Turn remote control on, then scan a code with the phone's camera.
settings-remote-pair-ready = Scan a code with the phone's camera, then approve the phone here.
settings-remote-pair-connected = { $count ->
        [one] { $count } phone connected.
       *[other] { $count } phones connected.
    } Scan a code with another phone's camera to add it.
settings-remote-pair-button = Pair a phone…
settings-remote-pair-no-window = Open a Sheprd window to pair a device.
settings-remote-pair-dialog-open = Close the open dialog, then pair the device.

settings-remote-history-reload-failed = Saved; host daemon reload not queued: { $error }
