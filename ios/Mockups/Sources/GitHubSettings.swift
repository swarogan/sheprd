import SwiftUI

// Settings > GitHub: one account per host. github.com can be signed in on
// the phone itself (device flow, Sheprd's GitHub App); otherwise, and for
// every Enterprise host, a paired computer lends its account.
// State 0: signed out here, MacBook Pro lends github.com.
// State 1: signed in on this iPhone; the computers' accounts are the fallback.
// A: a GitHub section in Settings, one row per host; 2 opens github.com,
//    3 opens the Enterprise host.
// B: a GitHub page of its own, a section per host with its facts.
// C: account cards, the sign-in button inside github.com's card.
// D: by source: this iPhone first, then what each computer lends.

struct GHSettingsFrame<Content: View, Pushed: View>: View {
    var title: String? = nil
    /// Shown as pushed on top, with a back button, when there is one.
    var pushed: Pushed?
    @ViewBuilder var content: Content

    init(title: String? = nil, pushed: Pushed? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.pushed = pushed
        self.content = content()
    }

    var body: some View {
        GHTabs(selected: .settings) {
            NavigationStack(path: .constant(pushed == nil ? [] : [1])) {
                List { content }
                    .listSectionSpacing(14)
                    .navigationTitle(title ?? "")
                    .navigationBarTitleDisplayMode(.inline)
                    .navigationDestination(for: Int.self) { _ in pushed }
            }
        }
    }
}

extension GHSettingsFrame where Pushed == EmptyView {
    init(title: String? = nil, @ViewBuilder content: () -> Content) {
        self.init(title: title, pushed: nil, content: content)
    }
}

/// What the screens say about where accounts come from.
enum GHCopy {
    static let fallback = "Your computers' accounts are the fallback: when this iPhone is signed out or its sign-in fails, it reads with the one MacBook Pro lends."
    static let enterprise = "Enterprise hosts come only from gh on a computer: Sheprd's GitHub App exists on github.com alone."
    static let revoke = "Signing out deletes this iPhone's tokens. To revoke Sheprd's access itself, use Applications in your GitHub settings."
}

private let enterprise = GH.accounts[1]

private struct HostLine: View {
    var tint: Color
    var login: String
    var title: String
    var detail: String
    var detailImage: String
    var size: CGFloat = 36

    var body: some View {
        HStack(spacing: 12) {
            GHAvatar(person: GHPerson(login: login, tint: tint), size: size)
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: title).lineLimit(1).truncationMode(.middle)
                GHLabel(detail, systemImage: detailImage)
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }
}

private struct SignInButton: View {
    var body: some View {
        Button("Sign In on This iPhone", systemImage: "person.badge.key") {}
    }
}

private struct EnterpriseLine: View {
    var size: CGFloat = 36

    var body: some View {
        HostLine(tint: enterprise.tint, login: enterprise.login, title: enterprise.host,
                 detail: "octocat-acme from studio · gh", detailImage: "macstudio", size: size)
    }
}

// MARK: A

struct GHSettingsRows: View {
    var state: Int

    var body: some View {
        GHSettingsFrame(pushed: pushed) {
            Section("Computers") {
                ForEach(["MacBook Pro", "studio", "build-01"], id: \.self) { name in
                    NavigationLink {} label: { Text(name) }
                }
            }
            Section {
                NavigationLink {} label: {
                    if state == 0 {
                        HostLine(tint: .indigo, login: "octocat", title: "github.com",
                                 detail: "Using octocat from MacBook Pro", detailImage: "laptopcomputer")
                    } else {
                        HostLine(tint: .indigo, login: "octocat", title: "octocat",
                                 detail: "Signed in on this iPhone", detailImage: "iphone")
                    }
                }
                if state == 0 { SignInButton() }
                NavigationLink {} label: { EnterpriseLine() }
            } header: {
                Text("GitHub")
            } footer: {
                Text(state == 0 ? "Signing in here is for github.com. \(GHCopy.enterprise)" : GHCopy.fallback)
            }
            Section("Connection") {
                LabeledContent("Relays", value: "1")
            }
        }
    }

    private var pushed: AnyView? {
        switch state {
        case 2: AnyView(dotComPage)
        case 3: AnyView(enterprisePage)
        default: nil
        }
    }

    private func header(_ login: String, _ name: String, _ tint: Color, _ caption: String, _ image: String) -> some View {
        Section {
            VStack(spacing: 6) {
                GHAvatar(person: GHPerson(login: login, tint: tint), size: 56)
                Text(login).font(.title3.weight(.semibold))
                Text(name).font(.subheadline).foregroundStyle(.secondary)
                GHLabel(caption, systemImage: image).font(.caption).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
            .listRowBackground(Color.clear)
        }
    }

    private var dotComPage: some View {
        List {
            header("octocat", "The Octocat", .indigo, "Signed in on this iPhone", "iphone")
            Section {
                LabeledContent("Token", value: "Renews in 7 h")
                Button("Sign Out", role: .destructive) {}
                Link(destination: URL(string: "https://github.com/settings/apps/authorizations")!) {
                    Label("Review Access on GitHub", systemImage: "arrow.up.forward.square")
                }
            } footer: {
                Text(GHCopy.revoke)
            }
            Section {
                LabeledContent("MacBook Pro", value: "octocat · Sheprd")
                LabeledContent("studio", value: "octocat · gh")
            } header: {
                Text("Fallback")
            } footer: {
                Text("Used when this iPhone is signed out or its sign-in fails.")
            }
        }
        .ghScreen("github.com")
    }

    private var enterprisePage: some View {
        List {
            header(enterprise.login, enterprise.name, enterprise.tint, "From studio", "macstudio")
            Section {
                LabeledContent("Lent by", value: "studio")
                LabeledContent("Source", value: "GitHub CLI (gh)")
                LabeledContent("Expires", value: "Never")
            } footer: {
                Text("\(GHCopy.enterprise) To change the account, run gh auth login on studio.")
            }
            Section {
                Button("Forget on This iPhone", role: .destructive) {}
            } footer: {
                Text("To stop lending it, turn off Share GitHub sign-in for this phone in Sheprd's Remote control settings on studio.")
            }
        }
        .ghScreen(enterprise.host)
    }
}

// MARK: B

struct GHSettingsPage: View {
    var state: Int

    var body: some View {
        GHSettingsFrame(title: "GitHub") {
            if state == 0 { signedOut } else { signedIn }
            Section {
                EnterpriseLine(size: 44)
                LabeledContent("Lent by") { GHLabel("studio", systemImage: "macstudio") }
                LabeledContent("Source", value: "GitHub CLI (gh)")
                LabeledContent("Expires", value: "Never")
            } header: {
                Text(enterprise.host).textCase(nil)
            } footer: {
                Text(GHCopy.enterprise)
            }
        }
    }

    @ViewBuilder private var signedOut: some View {
        Section {
            HostLine(tint: .indigo, login: "octocat", title: "Using octocat from MacBook Pro",
                     detail: "Sheprd sign-in · renews in 6 h", detailImage: "laptopcomputer", size: 44)
            Button {} label: {
                Text("Sign In on This iPhone").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
        } header: {
            Text("github.com").textCase(nil)
        } footer: {
            Text("This iPhone's own sign-in comes first; MacBook Pro's stays as the fallback.")
        }
    }

    @ViewBuilder private var signedIn: some View {
        Section {
            HostLine(tint: .indigo, login: "octocat", title: "octocat",
                     detail: "Signed in on this iPhone", detailImage: "iphone", size: 44)
            LabeledContent("Token", value: "Renews in 7 h")
            LabeledContent("Fallback") { GHLabel("MacBook Pro", systemImage: "laptopcomputer") }
            Button("Sign Out", role: .destructive) {}
            Link(destination: URL(string: "https://github.com/settings/apps/authorizations")!) {
                Label("Review Access on GitHub", systemImage: "arrow.up.forward.square")
            }
        } header: {
            Text("github.com").textCase(nil)
        } footer: {
            Text(GHCopy.fallback)
        }
    }
}

// MARK: C

struct GHSettingsCards: View {
    var state: Int

    var body: some View {
        GHSettingsFrame(title: "GitHub") {
            Section {
                dotCom
                Card(person: GHPerson(login: enterprise.login, tint: enterprise.tint), name: enterprise.name,
                     host: enterprise.host, chips: [("from studio", "macstudio"), ("gh · no expiry", "infinity")]) {
                    Text("No sign-in here: \(GHCopy.enterprise)")
                        .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 5, leading: 0, bottom: 5, trailing: 0))
            .listRowSeparator(.hidden)
        }
    }

    @ViewBuilder private var dotCom: some View {
        if state == 0 {
            Card(person: GH.octocat, name: "The Octocat", host: "github.com",
                 chips: [("from MacBook Pro", "laptopcomputer"), ("renews in 6 h", "clock")]) {
                Text("Using octocat from MacBook Pro. Sign in here to read with this iPhone's own sign-in.")
                    .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                Button {} label: {
                    Label("Sign In on This iPhone", systemImage: "person.badge.key").frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
            }
        } else {
            Card(person: GH.octocat, name: "The Octocat", host: "github.com",
                 chips: [("This iPhone", "iphone"), ("renews in 7 h", "clock")]) {
                Text("Fallback: MacBook Pro, studio. \(GHCopy.revoke)")
                    .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                HStack {
                    Button("Sign Out", role: .destructive) {}.buttonStyle(.bordered)
                    Spacer()
                    Link("Access on GitHub", destination: URL(string: "https://github.com/settings/apps/authorizations")!)
                        .font(.subheadline)
                }
            }
        }
    }
}

private struct Card<Footer: View>: View {
    var person: GHPerson
    var name: String
    var host: String
    var chips: [(String, String)]
    @ViewBuilder var footer: Footer

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                GHAvatar(person: person, size: 48)
                VStack(alignment: .leading, spacing: 2) {
                    Text(person.login).font(.headline).lineLimit(1)
                    Text(name).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 4)
                Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
            }
            Text(verbatim: host).font(.caption.monospaced()).foregroundStyle(.secondary)
            HStack(spacing: 6) {
                ForEach(chips, id: \.0) { chip in
                    GHLabel(chip.0, systemImage: chip.1)
                        .padding(.horizontal, 8).padding(.vertical, 3).background(.fill.tertiary, in: .capsule)
                }
            }
            .font(.caption).lineLimit(1)
            footer
        }
        .padding(14)
        .background(Color(.secondarySystemGroupedBackground), in: .rect(cornerRadius: 22))
    }
}

// MARK: D

struct GHSettingsByComputer: View {
    var state: Int

    var body: some View {
        GHSettingsFrame(title: "GitHub") {
            Section {
                if state == 0 {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Not signed in")
                        GHLabel("Using octocat from MacBook Pro", systemImage: "laptopcomputer")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    SignInButton()
                } else {
                    HStack {
                        HostLine(tint: .indigo, login: "octocat", title: "octocat",
                                 detail: "Signed in on this iPhone", detailImage: "iphone")
                        Spacer(minLength: 4)
                        Text("In use").font(.caption.weight(.semibold)).foregroundStyle(.green)
                    }
                    LabeledContent("github.com", value: "Renews in 7 h")
                    Button("Sign Out", role: .destructive) {}
                }
            } header: {
                Label("This iPhone", systemImage: "iphone").textCase(nil)
            } footer: {
                if state == 1 { Text("\(GHCopy.fallback) \(GHCopy.revoke)") }
            }
            Section {
                Lends(host: "github.com", login: "octocat", source: "Sheprd · renews in 6 h",
                      use: state == 0 ? .inUse : .fallback)
            } header: {
                Label("MacBook Pro", systemImage: "laptopcomputer").textCase(nil)
            }
            Section {
                Lends(host: enterprise.host, login: enterprise.login, source: "gh · no expiry", use: .inUse)
                Lends(host: "github.com", login: "octocat", source: "gh · no expiry", use: .fallback)
            } header: {
                Label("studio", systemImage: "macstudio").textCase(nil)
            } footer: {
                Text("\(GHCopy.enterprise) When two computers lend one host, the one reached most recently is used.")
            }
        }
    }
}

private enum Use { case inUse, fallback }

private struct Lends: View {
    var host: String
    var login: String
    var source: String
    var use: Use

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: host).lineLimit(1).truncationMode(.middle)
                Text(verbatim: "\(login) · \(source)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 4)
            Text(use == .inUse ? "In use" : "Fallback").font(.caption.weight(.semibold))
                .foregroundStyle(use == .inUse ? AnyShapeStyle(.green) : AnyShapeStyle(.secondary))
        }
    }
}
