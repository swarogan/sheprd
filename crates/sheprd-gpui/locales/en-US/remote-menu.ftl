# Remote control in the title bar: the button, and the list of paired
# phones it opens.

# What a screen reader says for the button.
remote-menu-pair-label = Pair a phone for remote control
remote-menu-phones-label =
    { $count ->
        [0] Remote control phones, none connected
       *[other] Remote control phones, { $count } connected
    }
# The button's text before any phone is paired.
remote-menu-pair-phone = Pair phone
# The list's heading, in capitals.
remote-menu-phones = PHONES
remote-menu-pair = Pair a phone…
remote-menu-settings = Settings
# Over the list while the service runs a build without a developer ID and
# the macOS firewall is on.
remote-menu-firewall = After a rebuild, the next restart blocks phones until you click Allow here.
remote-menu-disconnect = Disconnect
# What a screen reader says for a phone's Disconnect button; $device is
# its name.
remote-menu-disconnect-device = Disconnect { $device }
