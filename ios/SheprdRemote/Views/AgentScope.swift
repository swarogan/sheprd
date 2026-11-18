import SheprdRemoteCore
import SwiftUI

/// The repository or workspace the agent lists show, kept on this phone and
/// shared by its windows. Read it through `AppModel.agentScope`: the
/// workspace may have closed since.
@MainActor
@Observable
final class AgentScopeStore {
    private static let key = "agentScope"

    var scope: AgentScope? {
        didSet { UserDefaults.standard.set(scope.flatMap { try? JSONEncoder().encode($0) }, forKey: Self.key) }
    }

    init() {
        scope = UserDefaults.standard.data(forKey: Self.key).flatMap { try? JSONDecoder().decode(AgentScope.self, from: $0) }
    }
}

extension AppModel {
    /// The scope the agent lists show, while a workspace on a computer the
    /// tag filter leaves still falls in it. One the tag or computer chosen
    /// since hides would otherwise empty the list, with no choice left in
    /// the menu to clear it; it applies again once its computer shows.
    var agentScope: AgentScope? {
        AgentScope.active(agentScopeStore.scope, among: shownHomes)
    }

    /// Whether the agent lists show what runs in `workspace` on `computer`:
    /// a computer the tag filter leaves, in the chosen repository or
    /// workspace. Read once per list, not per row.
    func agentListFilter() -> (Computer, Home.Workspace) -> Bool {
        let shown = tagFilter.shown(among: computers)
        let scope = agentScope
        return { computer, workspace in
            shown.contains(computer.device) && scope?.contains(workspace, on: computer.device) != false
        }
    }

    /// The homes of the computers the tag filter leaves, which the scopes
    /// offered and applied come from.
    var shownHomes: [(device: String, home: Home)] {
        let shown = tagFilter.shown(among: computers)
        return fleet.machines.compactMap { machine in
            guard shown.contains(machine.id), let home = machine.home else { return nil }
            return (machine.id, home)
        }
    }
}

/// Narrows Home's and the iPad sidebar's agents, from beside their sort
/// menu: to the computers with a tag, to one computer, and to a repository
/// or one of its workspaces. Filled while any of them applies.
struct AgentFilterMenu: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let computers = model.computers
        let tags = ComputerTags.all(in: computers)
        let tag = ComputerTags.active(model.tagFilter.tag, among: computers)
        let device = ComputerTags.active(device: model.tagFilter.device, tag: tag, among: computers)
        let hosts = computers.filter { $0.matches(tag: tag) }
        let scope = model.agentScope
        let choices = AgentScope.choices(among: model.shownHomes)
        let filtered = tag != nil || device != nil || scope != nil
        Menu {
            Button("Show All Agents", systemImage: "xmark.circle") {
                withAnimation(.snappy) {
                    model.tagFilter.tag = nil
                    model.tagFilter.device = nil
                    model.agentScopeStore.scope = nil
                }
            }
            .disabled(!filtered)
            if !tags.isEmpty {
                Section("Tags") {
                    ForEach(tags, id: \.self) { name in
                        toggle(name, on: name == tag) { model.tagFilter.choose(tag: $0 ? name : nil, among: computers) }
                    }
                }
            }
            if hosts.count > 1 {
                Section("Computers") {
                    ForEach(hosts, id: \.device) { computer in
                        toggle(model.displayName(computer), on: computer.device == device) {
                            model.tagFilter.device = $0 ? computer.device : nil
                        }
                    }
                }
            }
            if !choices.isEmpty {
                Section("Repositories") {
                    ForEach(choices) { choice in
                        let chosen = scope == choice.scope || choice.places.contains { $0.scope == scope }
                        Menu {
                            option("All of \(choice.name)", scope: choice.scope, active: scope)
                            Section {
                                ForEach(choice.places) { place in
                                    option(label(place, in: choice), scope: place.scope, active: scope)
                                }
                            }
                        } label: {
                            Label(choice.name, systemImage: chosen ? "checkmark" : "folder")
                        }
                    }
                }
            }
        } label: {
            Image(systemName: filtered ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle")
        }
        .accessibilityLabel(filtered ? "Filter agents, filtered" : "Filter agents")
        .disabled(!filtered && tags.isEmpty && hosts.count < 2 && choices.isEmpty)
    }

    private func toggle(_ title: String, on: Bool, set: @escaping (Bool) -> Void) -> some View {
        Toggle(title, isOn: Binding(get: { on }, set: { value in withAnimation(.snappy) { set(value) } }))
    }

    private func option(_ title: String, scope: AgentScope, active: AgentScope?) -> some View {
        toggle(title, on: active == scope) { model.agentScopeStore.scope = $0 ? scope : nil }
    }

    /// The workspace's branch when it is the repository's own checkout, as
    /// the Workspaces tab shows it, and its computer when the repository is
    /// on more than one.
    private func label(_ place: AgentScope.Place, in choice: AgentScope.Choice) -> String {
        let workspace = place.workspace
        let name = workspace.label == choice.name ? workspace.branch ?? workspace.label : workspace.label
        guard Set(choice.places.map(\.device)).count > 1,
              let computer = model.computers.first(where: { $0.device == place.device }) else { return name }
        return "\(name) · \(model.displayName(computer))"
    }
}
