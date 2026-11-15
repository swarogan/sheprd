import SheprdRemoteCore
import SwiftUI
import UIKit

/// Settings > Relays: the relays this phone keeps, whether each answers,
/// and a field to add one. It works with no computer in reach: the relays
/// paired computers named are already here, and any other is typed.
struct RelaysView: View {
    @Environment(AppModel.self) private var model
    @State private var address = ""
    @State private var problem: String?
    /// Whether each relay answered, for as long as the screen shows.
    @State private var reach: [RelayEndpoint: RelayReach] = [:]
    @FocusState private var typing: Bool

    var body: some View {
        let relays = model.savedRelays.relays
        List {
            Section {
                if relays.isEmpty {
                    Text("No relay yet. A computer that listens on relays adds them here the next time this \(UIDevice.current.model) reaches it on its network.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                ForEach(relays, id: \.self) { relay in
                    let naming = model.computers(naming: relay)
                    RelayRow(relay: relay, reach: reach[relay], naming: naming.map(model.displayName))
                        // A relay a computer names comes back with it, so
                        // only the others can be removed.
                        .deleteDisabled(!naming.isEmpty)
                }
                .onDelete { offsets in
                    for relay in offsets.map({ relays[$0] }) { model.removeRelay(relay) }
                }
            } header: {
                Text("Relays")
            } footer: {
                if !relays.isEmpty {
                    Text("Each computer is tried on the relays it named, then on the others here. A relay sees only encrypted traffic.")
                }
            }
            Section {
                // Verbatim: a localized key that is a URL is drawn as a link.
                TextField("Relay address", text: $address, prompt: Text(verbatim: "https://relay.example.org"))
                    .keyboardType(.URL)
                    .textContentType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.done)
                    .focused($typing)
                    .onSubmit(add)
                Button("Add Relay", systemImage: "plus", action: add)
                    .disabled(address.trimmingCharacters(in: .whitespaces).isEmpty)
                if let problem {
                    Label(problem, systemImage: "exclamationmark.triangle")
                        .font(.footnote)
                        .foregroundStyle(.orange)
                }
            } header: {
                Text("Add a relay")
            } footer: {
                Text("Type the address of a relay a paired computer listens on, to reach that computer through it. A computer not paired yet is still paired nearby or on its network first.")
            }
        }
        .navigationTitle("Relays")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if !relays.isEmpty { EditButton() }
        }
        .refreshable { await check(relays, again: true) }
        // Again whenever the list changes: a relay just added is checked.
        .task(id: relays) { await check(relays, again: false) }
    }

    private func add() {
        switch model.addRelay(address) {
        case .added:
            address = ""
            problem = nil
            typing = false
        case .known:
            address = ""
            problem = "That relay is already here."
        case .invalid:
            problem = "That is not a relay's address. Type one such as https://relay.example.org."
        case .full:
            problem = "Remove a relay before adding another: \(SavedRelays.limit) are kept at most."
        }
    }

    /// Asks every relay whether it answers, all at once.
    private func check(_ relays: [RelayEndpoint], again: Bool) async {
        if again { reach = [:] }
        await withTaskGroup(of: (RelayEndpoint, RelayReach).self) { group in
            for relay in relays where reach[relay] == nil {
                group.addTask { (relay, await RelayReach.probe(relay)) }
            }
            for await (relay, answer) in group { reach[relay] = answer }
        }
    }
}

/// One relay: a dot for whether it answers, its address, then that in
/// words and which computers named it.
private struct RelayRow: View {
    var relay: RelayEndpoint
    var reach: RelayReach?
    var naming: [String]

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: "circle.fill")
                .font(.system(size: 9))
                .foregroundStyle(color)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(relay.description).lineLimit(1).truncationMode(.middle)
                Text(line).font(.caption).foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private var color: Color {
        switch reach {
        case .reachable: .green
        case .noWebSocket, .notARelay, .unreachable: .red
        case nil: Color(.tertiaryLabel)
        }
    }

    private var status: String {
        switch reach {
        case .reachable: "Reachable"
        case .noWebSocket: "No WebSocket, which this \(UIDevice.current.model) needs"
        case .notARelay: "Not a relay"
        case .unreachable: "No answer"
        case nil: "Checking…"
        }
    }

    private var line: String {
        guard !naming.isEmpty else { return status }
        return "\(status) · \(naming.formatted(.list(type: .and)))"
    }
}
