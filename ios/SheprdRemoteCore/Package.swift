// swift-tools-version: 6.0
// The app's logic that needs no UIKit, ActivityKit, or Keychain: pairing
// links, fingerprints, base64url, session signatures, the Live Activity
// content, sealed notifications, the API's JSON, and the transcript
// reducer. It builds for macOS too, so `swift test` runs it without a
// simulator.
//
// The other targets are split the way the Rust sub-crates are. `SheprdTunnel`
// sits below Core, since a saved relay record checks the identity it pins;
// the rest sit above it and depend on `SheprdRemoteCore`, never the other
// way. Only the app links those, not the widgets, watch, or extensions,
// which need Core alone.
//
// - `SheprdGitHub`: the phone's own GitHub client: hosts, leased and
//   signed-in tokens, the REST and GraphQL reads, pull requests, checks, and
//   their logs.
// - `SheprdNearby`: nearby pairing's exchange, framing, short code, and the
//   Bluetooth sightings it scans; the radio itself stays in the app.
// - `SheprdRelay`: reaching a computer through Sheprd relays: endpoints,
//   sessions, the local proxy, racing, nearness, and the saved list.
// - `SheprdTerminal`: a terminal tab's input and selection: keys, echo,
//   composition, dictation, handles, cells, and waiting.
// - `SheprdTunnel`: the end-to-end tunnel inside a relay stream, checked
//   against the Rust `sheprd-tunnel` crate's vectors.
// - `SheprdVoice`: voice commands: the parser, names, plan, drafts, and
//   follow-ups.

import PackageDescription

let package = Package(
    name: "SheprdRemoteCore",
    platforms: [.iOS("26.0"), .macOS("26.0"), .watchOS("26.0")],
    products: [
        // keep-sorted start
        .library(name: "SheprdGitHub", targets: ["SheprdGitHub"]),
        .library(name: "SheprdNearby", targets: ["SheprdNearby"]),
        .library(name: "SheprdRelay", targets: ["SheprdRelay"]),
        .library(name: "SheprdRemoteCore", targets: ["SheprdRemoteCore"]),
        .library(name: "SheprdTerminal", targets: ["SheprdTerminal"]),
        .library(name: "SheprdTunnel", targets: ["SheprdTunnel"]),
        .library(name: "SheprdVoice", targets: ["SheprdVoice"]),
        // keep-sorted end
    ],
    targets: [
        // keep-sorted start
        .target(name: "SheprdGitHub", dependencies: ["SheprdRemoteCore"]),
        .target(name: "SheprdNearby", dependencies: ["SheprdRemoteCore"]),
        .target(name: "SheprdRelay", dependencies: ["SheprdRemoteCore", "SheprdTunnel"]),
        .target(name: "SheprdRemoteCore", dependencies: ["SheprdTunnel"]),
        .target(name: "SheprdTerminal", dependencies: ["SheprdRemoteCore"]),
        .target(name: "SheprdTunnel"),
        .target(name: "SheprdVoice", dependencies: ["SheprdRemoteCore"]),
        // keep-sorted end
        // keep-sorted start
        .testTarget(
            name: "SheprdGitHubTests",
            dependencies: ["SheprdGitHub", "SheprdRemoteCore"],
            resources: [.copy("Fixtures")]
        ),
        .testTarget(
            name: "SheprdNearbyTests",
            dependencies: ["SheprdNearby", "SheprdRemoteCore"],
            resources: [.copy("Fixtures")]
        ),
        .testTarget(
            name: "SheprdRelayTests",
            dependencies: ["SheprdRelay", "SheprdRemoteCore", "SheprdTunnel"],
            resources: [.copy("Fixtures")]
        ),
        .testTarget(
            name: "SheprdRemoteCoreTests",
            dependencies: ["SheprdRemoteCore"],
            resources: [.copy("Fixtures")]
        ),
        .testTarget(name: "SheprdTerminalTests", dependencies: ["SheprdRemoteCore", "SheprdTerminal"]),
        .testTarget(
            name: "SheprdTunnelTests",
            dependencies: ["SheprdTunnel"],
            resources: [.copy("Fixtures")]
        ),
        .testTarget(
            name: "SheprdVoiceTests",
            dependencies: ["SheprdRemoteCore", "SheprdVoice"],
            resources: [.copy("Fixtures")]
        ),
        // keep-sorted end
    ]
)
