import SheprdGitHub
import SheprdRemoteCore
import SwiftUI
import UIKit

/// Settings > GitHub: a section per host with its account and where its
/// token comes from. github.com can be signed in on this phone itself;
/// otherwise, and for every Enterprise host, a paired computer lends its
/// account, which stays the fallback.
struct GitHubSettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var state = GitHubSignInState.idle
    @State private var ownExpiry: Date?
    @State private var leases: [GitHubLease] = []
    @State private var signingIn = false
    @State private var confirmingSignOut = false
    @State private var problem: String?
    #if DEBUG
    @State private var sample: [GitHubHostAccount]?
    #endif

    private var signIn: GitHubSignIn? {
        get async { await GitHubReads.reader.signIn }
    }

    var body: some View {
        List {
            ForEach(accounts) { account in
                if account.signsIn {
                    GitHubComSection(account: account, signIn: { signingIn = true }, signOut: { confirmingSignOut = true })
                } else {
                    LentHostSection(account: account)
                }
            }
            if let problem {
                Section { Text(problem).font(.footnote).foregroundStyle(.secondary) }
            }
        }
        .listSectionSpacing(14)
        .navigationTitle("GitHub")
        .navigationBarTitleDisplayMode(.inline)
        .task { await follow() }
        .task(id: state) { await reload() }
        .sheet(isPresented: $signingIn, onDismiss: { Task { await reload() } }) {
            GitHubSignInSheetView()
        }
        .confirmationDialog("Sign out of GitHub on this \(UIDevice.current.model)?", isPresented: $confirmingSignOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) { Task { await signOut() } }
        } message: {
            Text("Signing out deletes this \(UIDevice.current.model)'s tokens. To revoke Sheprd's access itself, use Applications in your GitHub settings.")
        }
        #if DEBUG
        .task { sample = GitHubSample.settings() }
        .task { if UserDefaults.standard.string(forKey: "gitHubSignIn") != nil { signingIn = true } }
        #endif
    }

    private var accounts: [GitHubHostAccount] {
        #if DEBUG
        if let sample { return sample }
        #endif
        return GitHubHostAccount.accounts(own: state, ownExpiry: ownExpiry, leases: leases, lendings: lendings)
    }

    /// What each computer's latest `home` lends, the most recently reached
    /// first, as a lease would pick.
    private var lendings: [GitHubLending] {
        model.fleet.machines
            .sorted { ($0.updated ?? .distantPast) > ($1.updated ?? .distantPast) }
            .map { GitHubLending(computer: model.displayName($0.computer), accounts: $0.home?.githubHosts ?? []) }
    }

    private func follow() async {
        guard let signIn = await signIn else { return }
        GitHubDirect.shared.watchSignIn()
        for await next in await signIn.states() { state = next }
    }

    private func reload() async {
        ownExpiry = await signIn?.expiry
        leases = await GitHubReads.reader.tokens.all()
    }

    private func signOut() async {
        do {
            try await signIn?.signOut()
            problem = nil
        } catch {
            problem = "The sign-in could not be deleted from the Keychain."
        }
        // What was read with the account goes with it.
        GitHubDirect.shared.forgetDirectReads()
    }
}

/// github.com: the phone's own sign-in, or the account a computer lends
/// and a button to sign in here.
private struct GitHubComSection: View {
    var account: GitHubHostAccount
    var signIn: () -> Void
    var signOut: () -> Void

    var body: some View {
        Section {
            if let own = account.own {
                AccountLine(login: own.login, title: own.login, detail: "Signed in on this \(UIDevice.current.model)",
                            detailImage: UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone")
                LabeledContent("Token", value: GitHubTimes.renewal(own.expiresAt))
                LabeledContent("Fallback") {
                    if let lent = account.lent {
                        IconText(lent.computer, systemImage: "laptopcomputer")
                    } else {
                        Text("None")
                    }
                }
                Button("Sign Out", role: .destructive, action: signOut)
                if let url = URL(string: GitHubDeviceFlow.applicationsURL) {
                    Link(destination: url) {
                        Label("Review Access on GitHub", systemImage: "arrow.up.forward.square")
                    }
                }
            } else {
                if let lent = account.lent {
                    AccountLine(login: lent.login, title: "Using \(lent.login) from \(lent.computer)",
                                detail: GitHubTimes.lentDetail(lent), detailImage: "laptopcomputer")
                } else {
                    AccountLine(login: nil, title: "Not signed in", detail: "No computer lends an account", detailImage: "person.crop.circle.badge.questionmark")
                }
                Button(action: signIn) {
                    Text("Sign In on This \(UIDevice.current.model)").frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
            }
        } header: {
            Text(verbatim: account.host.name).textCase(nil)
        } footer: {
            Text(footer)
        }
    }

    private var footer: String {
        let device = UIDevice.current.model
        if account.problem == .refreshRejected {
            return "GitHub no longer accepts this \(device)'s sign-in: it was revoked, or expired. Sign in again to read GitHub directly."
        }
        if account.own != nil {
            guard let lent = account.lent else {
                return "Signing out deletes this \(device)'s tokens. To revoke Sheprd's access itself, use Applications in your GitHub settings."
            }
            return "Your computers' accounts are the fallback: when this \(device) is signed out or its sign-in fails, it reads with the one \(lent.computer) lends."
        }
        if let lent = account.lent {
            return "This \(device)'s own sign-in comes first; \(lent.computer)'s stays as the fallback."
        }
        return "Sign in to read pull requests and checks on this \(device) without a computer."
    }
}

/// An Enterprise host, or another host a computer lends: its account and
/// where it comes from. Sheprd's GitHub App exists on github.com alone, so
/// there is no signing in here.
private struct LentHostSection: View {
    var account: GitHubHostAccount

    var body: some View {
        Section {
            if let lent = account.lent {
                AccountLine(login: lent.login, title: account.host.name, detail: "\(lent.login) from \(lent.computer)\(lent.source == .gh ? " · gh" : "")",
                            detailImage: "laptopcomputer")
                LabeledContent("Lent by") { IconText(lent.computer, systemImage: "laptopcomputer") }
                LabeledContent("Source", value: GitHubTimes.source(lent.source))
                LabeledContent("Expires", value: GitHubTimes.expiry(lent))
            }
        } header: {
            Text(verbatim: account.host.name).textCase(nil)
        } footer: {
            Text("Enterprise hosts come only from gh on a computer: Sheprd's GitHub App exists on github.com alone.")
        }
    }
}

/// An account's initial, its title, and where it comes from.
private struct AccountLine: View {
    var login: String?
    var title: String
    var detail: String
    var detailImage: String

    var body: some View {
        HStack(spacing: 12) {
            PullRequestAvatar(login: login, size: 44)
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: title).lineLimit(1).truncationMode(.middle)
                IconText(detail, systemImage: detailImage)
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// How Settings words a token's source and time.
enum GitHubTimes {
    static func renewal(_ expires: Date?, now: Date = .now) -> String {
        guard let expires else { return "Does not expire" }
        return "Renews in \(span(expires.timeIntervalSince(now)))"
    }

    static func lentDetail(_ lent: GitHubHostAccount.Lent, now: Date = .now) -> String {
        switch lent.source {
        case .sheprd?:
            lent.expiresAt.map { "Sheprd sign-in · renews in \(span($0.timeIntervalSince(now)))" } ?? "Sheprd sign-in"
        case .gh?: "GitHub CLI (gh)"
        case .other?, nil: "Lent by \(lent.computer)"
        }
    }

    static func source(_ source: LeaseSource?) -> String {
        switch source {
        case .sheprd?: "Sheprd sign-in"
        case .gh?: "GitHub CLI (gh)"
        case .other?: "Other"
        case nil: "Not borrowed yet"
        }
    }

    static func expiry(_ lent: GitHubHostAccount.Lent, now: Date = .now) -> String {
        if let expires = lent.expiresAt { return "In \(span(expires.timeIntervalSince(now)))" }
        return lent.source == nil ? "Unknown" : "Never"
    }

    /// "6 h", "12 min", or "now".
    static func span(_ seconds: TimeInterval) -> String {
        switch seconds {
        case ..<60: "now"
        case ..<3600: "\(Int(seconds / 60)) min"
        default: "\(Int(seconds / 3600)) h"
        }
    }
}

/// Settings' row leading to the GitHub page, with the account in use.
struct GitHubSettingsRow: View {
    @State private var login: String?

    var body: some View {
        NavigationLink {
            GitHubSettingsView()
        } label: {
            LabeledContent {
                if let login { Text(verbatim: login) }
            } label: {
                Label("GitHub", systemImage: "arrow.triangle.pull")
            }
        }
        .task {
            guard let signIn = await GitHubReads.reader.signIn else { return }
            for await state in await signIn.states() {
                if case let .signedIn(name, _) = state { login = name } else { login = nil }
            }
        }
    }
}
