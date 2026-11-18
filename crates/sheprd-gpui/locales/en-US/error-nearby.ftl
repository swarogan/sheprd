# Failures relaying frames through sheprd-nearby, the Bluetooth helper for
# nearby pairing and presence.

error-nearby-malformed = a nearby frame is not JSON this relay reads
# $program is a file name, such as sheprd-remote-control.
error-nearby-missing = { $program } is not beside the app
error-nearby-no-state-dir = this computer has no state directory
# $pipe is a standard stream: stdin, stdout, or stderr.
error-nearby-pipe = the nearby helper has no { $pipe } pipe
error-nearby-spawn = could not start the nearby helper
# $limit is a size in bytes.
error-nearby-too-large = a nearby frame is over { $limit } bytes
