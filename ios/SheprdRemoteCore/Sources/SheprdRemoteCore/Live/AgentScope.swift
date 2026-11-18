import Foundation

/// Which agents the agent lists show, among the computers the tag filter
/// leaves: those of one repository, on every computer that has it, or of
/// one workspace. No scope shows them all.
public enum AgentScope: Codable, Hashable, Sendable {
    /// A repository by name: each computer keeps its own checkout, so the
    /// name is all two computers' copies share.
    case repository(String)
    case workspace(device: String, id: String)

    /// Whether the agents of `workspace` on `device` show.
    public func contains(_ workspace: Home.Workspace, on device: String) -> Bool {
        switch self {
        case let .repository(name): workspace.repositoryName == name
        case let .workspace(scoped, id): scoped == device && workspace.id == id
        }
    }

    /// `scope` while a workspace in `homes` still falls in it. One closed
    /// since would otherwise leave an empty list with no choice left to
    /// clear it in the menu.
    public static func active(_ scope: AgentScope?, among homes: [(device: String, home: Home)]) -> AgentScope? {
        guard let scope else { return nil }
        let present = homes.contains { device, home in home.workspaces.contains { scope.contains($0, on: device) } }
        return present ? scope : nil
    }

    /// A repository to offer, with its workspaces that run an agent.
    public struct Choice: Hashable, Sendable, Identifiable {
        public var name: String
        public var places: [Place]

        public var id: String { name }
        public var scope: AgentScope { .repository(name) }
    }

    /// A workspace to offer, on the computer it is on.
    public struct Place: Hashable, Sendable, Identifiable {
        public var device: String
        public var workspace: Home.Workspace

        public var id: AgentScope { scope }
        public var scope: AgentScope { .workspace(device: device, id: workspace.id) }
    }

    /// The repositories with an agent in `homes`, alphabetically, each with
    /// its workspaces in the computers' order: a scope with no agent in it
    /// would only empty the list.
    public static func choices(among homes: [(device: String, home: Home)]) -> [Choice] {
        var choices: [String: [Place]] = [:]
        for (device, home) in homes {
            for workspace in home.workspaces where workspace.runsAgent {
                choices[workspace.repositoryName, default: []].append(Place(device: device, workspace: workspace))
            }
        }
        return choices
            .map { Choice(name: $0.key, places: $0.value) }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}

extension Home.Workspace {
    /// The repository it is a checkout of, or its own label when its
    /// computer names none.
    public var repositoryName: String { worktree?.label ?? label }

    /// A chat, or an agent in a terminal tab, runs in it.
    var runsAgent: Bool { !chats.isEmpty || terminals.contains { $0.agent != nil } }
}
