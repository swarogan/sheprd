import SwiftUI

// Signing in to github.com on the phone: GitHub's device flow with
// Sheprd's GitHub App. The phone shows the code, opens
// github.com/login/device, and polls until the person approves.
// State 0: the code, ready to copy. 1: waiting, with the code's expiry.
// 2: the code expired. 3: GitHub denied it. 4: signed in, and an
// organization that has not installed the App reads public repositories only.
// A: a sheet over Settings > GitHub.
// B: a pushed page of numbered steps; 4 is back on the GitHub page.
// C: inline in Settings > GitHub; 4 is a banner on the pull request list.

private let code = "WDJB-MJHT"
private let deviceURL = URL(string: "https://github.com/login/device")!
private let installURL = URL(string: "https://github.com/apps/sheprd/installations/new")!

private enum SignInStep {
    case code, waiting, expired, denied, done

    init(_ state: Int) {
        self = [.code, .waiting, .expired, .denied, .done][min(max(state, 0), 4)]
    }

    var failed: Bool { self == .expired || self == .denied }

    var problem: (title: String, detail: String, symbol: String) {
        if self == .expired {
            return ("This code expired", "Codes last 15 minutes. Get a new one and enter it on GitHub.", "clock.badge.xmark")
        }
        return ("GitHub denied the sign-in", "Cancel was chosen on GitHub, or an owner refused the request. Nothing was saved on this iPhone.", "xmark.octagon")
    }
}

/// The code in large letters, with its copy button.
private struct CodeBox: View {
    var step: SignInStep
    var compact = false

    var body: some View {
        HStack(spacing: 10) {
            Text(verbatim: code)
                .font(.system(compact ? .title2 : .largeTitle, design: .monospaced).weight(.semibold))
                .tracking(compact ? 1 : 3)
                .strikethrough(step == .expired)
                .foregroundStyle(step.failed ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.primary))
                .lineLimit(1).minimumScaleFactor(0.6)
            Spacer(minLength: 0)
            Button("Copy", systemImage: "doc.on.doc") {}
                .labelStyle(.iconOnly)
                .buttonStyle(.bordered)
                .disabled(step.failed)
        }
        .padding(.horizontal, compact ? 0 : 18).padding(.vertical, compact ? 0 : 14)
        .background(compact ? AnyShapeStyle(.clear) : AnyShapeStyle(.fill.tertiary), in: .rect(cornerRadius: 16))
    }
}

/// The time left on the code, as a ring that empties.
private struct Countdown: View {
    var left = "14:32"
    var fraction = 0.97

    var body: some View {
        ZStack {
            Circle().stroke(.fill.secondary, lineWidth: 4)
            Circle().trim(from: 0, to: fraction).stroke(.tint, style: StrokeStyle(lineWidth: 4, lineCap: .round))
                .rotationEffect(.degrees(-90))
            Text(left).font(.caption2.monospacedDigit().weight(.semibold))
        }
        .frame(width: 46, height: 46)
    }
}

/// Organizations the account belongs to, and which the App can read.
private struct Organizations: View {
    var body: some View {
        Section {
            org("sheprd-labs", .teal, "All repositories", "checkmark.circle.fill", .green)
            org("octo-org", .orange, "Public repositories only", "exclamationmark.triangle.fill", .orange)
            Link(destination: installURL) {
                Label("Install Sheprd on octo-org", systemImage: "arrow.up.forward.square")
            }
        } header: {
            Text("Organizations")
        } footer: {
            Text("octo-org has not installed Sheprd's GitHub App, so this iPhone reads only its public repositories. An owner can install it; until then, its private pull requests come from your computers.")
        }
    }

    private func org(_ name: String, _ tint: Color, _ access: String, _ symbol: String, _ color: Color) -> some View {
        HStack(spacing: 12) {
            GHAvatar(person: GHPerson(login: name, tint: tint), size: 30)
            Text(name).lineLimit(1)
            Spacer(minLength: 6)
            GHLabel(access, systemImage: symbol).font(.caption).foregroundStyle(color).lineLimit(1)
        }
    }
}

// MARK: A

struct GHSignInSheet: View {
    var state: Int

    var body: some View {
        GHSettingsPage(state: 0)
            .sheet(isPresented: .constant(true)) {
                NavigationStack {
                    content
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button(step == .done ? "Done" : "Cancel") {}
                            }
                        }
                        .ghScreen(step == .done ? "Signed In" : "Sign In to GitHub")
                }
                .presentationDetents([.large])
            }
    }

    private var step: SignInStep { SignInStep(state) }

    @ViewBuilder private var content: some View {
        if step == .done {
            List {
                Section {
                    VStack(spacing: 6) {
                        GHAvatar(person: GH.octocat, size: 64)
                        Text("octocat").font(.title3.weight(.semibold))
                        GHLabel("Signed in on this iPhone", systemImage: "checkmark.seal.fill")
                            .font(.subheadline).foregroundStyle(.green)
                    }
                    .frame(maxWidth: .infinity)
                    .listRowBackground(Color.clear)
                }
                Organizations()
            }
        } else {
            ScrollView {
                VStack(spacing: 18) {
                    Image(systemName: step.failed ? step.problem.symbol : "person.badge.key.fill")
                        .font(.system(size: 44)).foregroundStyle(step.failed ? AnyShapeStyle(.red) : AnyShapeStyle(.tint))
                        .padding(.top, 12)
                    Text(step.failed ? step.problem.title : "Enter this code on GitHub")
                        .font(.title2.weight(.semibold)).multilineTextAlignment(.center)
                    Text(step.failed ? step.problem.detail : "Copy it, then open GitHub and paste it. Sheprd reads your pull requests and checks; it cannot change anything.")
                        .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                    CodeBox(step: step)
                    status
                    actions
                }
                .padding(.horizontal, 24)
            }
        }
    }

    @ViewBuilder private var status: some View {
        if step == .waiting {
            HStack(spacing: 10) {
                ProgressView()
                Text("Waiting for you to approve on GitHub").font(.subheadline)
                Spacer(minLength: 4)
                Text("Expires in 14:32").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
            .padding(12)
            .background(.fill.quaternary, in: .rect(cornerRadius: 12))
        }
    }

    @ViewBuilder private var actions: some View {
        if step.failed {
            Button {} label: {
                Text(step == .expired ? "Get a New Code" : "Try Again").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent).controlSize(.large)
        } else {
            VStack(spacing: 6) {
                Button {} label: {
                    Label("Open GitHub", systemImage: "safari").frame(maxWidth: .infinity)
                }
                .buttonStyle(step == .code ? AnyPrimitiveButtonStyle(.borderedProminent) : AnyPrimitiveButtonStyle(.bordered))
                .controlSize(.large)
                Text(verbatim: "github.com/login/device").font(.caption.monospaced()).foregroundStyle(.secondary)
            }
        }
    }
}

/// Picks between two button styles at run time.
private struct AnyPrimitiveButtonStyle: PrimitiveButtonStyle {
    private let make: (Configuration) -> AnyView

    init(_ style: some PrimitiveButtonStyle) {
        make = { AnyView(style.makeBody(configuration: $0)) }
    }

    func makeBody(configuration: Configuration) -> some View { make(configuration) }
}

// MARK: B

struct GHSignInSteps: View {
    var state: Int

    private var step: SignInStep { SignInStep(state) }

    var body: some View {
        GHSettingsFrame(pushed: page) {
            Section("GitHub") {
                Text("github.com")
            }
        }
    }

    @ViewBuilder private var page: some View {
        if step == .done {
            List {
                Section {
                    HStack(spacing: 12) {
                        GHAvatar(person: GH.octocat, size: 44)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("octocat")
                            GHLabel("Signed in on this iPhone", systemImage: "iphone")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    LabeledContent("Fallback") { GHLabel("MacBook Pro", systemImage: "laptopcomputer") }
                } header: {
                    Text("github.com").textCase(nil)
                }
                Organizations()
                Section {
                    Button("Sign Out", role: .destructive) {}
                }
            }
            .ghScreen("GitHub")
        } else {
            List {
                Section {
                    row(1, "Copy the code") { CodeBox(step: step, compact: true) }
                    row(2, "Open GitHub and paste it") {
                        Button {} label: { Label("Open GitHub", systemImage: "safari") }
                            .buttonStyle(.borderless).disabled(step.failed)
                            .foregroundStyle(step.failed ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.tint))
                    }
                    row(3, "Approve Sheprd") {
                        Text("Sheprd reads pull requests and checks; it cannot change anything.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                } footer: {
                    Text(verbatim: "github.com/login/device")
                }
                Section {
                    statusRow
                } footer: {
                    if step == .waiting {
                        Text("This iPhone asks GitHub every few seconds, as GitHub allows. You can leave this page; it keeps waiting until the code expires.")
                    }
                }
            }
            .ghScreen("Sign In to GitHub")
        }
    }

    @ViewBuilder private var statusRow: some View {
        switch step {
        case .waiting:
            HStack(spacing: 12) {
                ProgressView()
                Text("Waiting for GitHub")
                Spacer(minLength: 4)
                Text("14:32").font(.body.monospacedDigit()).foregroundStyle(.secondary)
            }
        case .expired, .denied:
            VStack(alignment: .leading, spacing: 4) {
                GHLabel(step.problem.title, systemImage: step.problem.symbol).foregroundStyle(.red)
                Text(step.problem.detail).font(.caption).foregroundStyle(.secondary)
            }
            Button(step == .expired ? "Get a New Code" : "Try Again", systemImage: "arrow.clockwise") {}
        default:
            LabeledContent("Code expires in", value: "15:00")
        }
    }

    private func row(_ number: Int, _ title: String, @ViewBuilder detail: () -> some View) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text("\(number)").font(.subheadline.weight(.bold)).foregroundStyle(.white)
                .frame(width: 24, height: 24)
                .background(step.failed ? AnyShapeStyle(.gray) : AnyShapeStyle(.tint), in: .circle)
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                detail()
            }
        }
        .padding(.vertical, 2)
    }
}

// MARK: C

struct GHSignInInline: View {
    var state: Int

    private var step: SignInStep { SignInStep(state) }

    var body: some View {
        if step == .done { pullRequests } else { settings }
    }

    private var settings: some View {
        GHSettingsFrame(title: "GitHub") {
            Section {
                HStack(spacing: 12) {
                    GHAvatar(person: GH.octocat, size: 36)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Signing in on this iPhone")
                        GHLabel("Using octocat from MacBook Pro", systemImage: "laptopcomputer")
                            .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
                HStack(spacing: 14) {
                    if step == .waiting { Countdown() }
                    VStack(alignment: .leading, spacing: 4) {
                        Text(step == .waiting ? "Enter on GitHub, waiting…" : "Your code").font(.caption).foregroundStyle(.secondary)
                        CodeBox(step: step, compact: true)
                    }
                }
                if step.failed {
                    VStack(alignment: .leading, spacing: 4) {
                        GHLabel(step.problem.title, systemImage: step.problem.symbol)
                            .foregroundStyle(.red).font(.subheadline.weight(.semibold))
                        Text(step.problem.detail).font(.caption).foregroundStyle(.secondary)
                    }
                    Button(step == .expired ? "Get a New Code" : "Try Again", systemImage: "arrow.clockwise") {}
                } else {
                    Link(destination: deviceURL) { Label("Open GitHub", systemImage: "safari") }
                }
                Button("Cancel", role: .cancel) {}.foregroundStyle(.secondary)
            } header: {
                Text("github.com").textCase(nil)
            } footer: {
                Text(step == .waiting ? "The ring shows how long the code lasts. Approve Sheprd on GitHub and this row turns into your account." : "Opens github.com/login/device. Paste the code there and approve Sheprd.")
            }
            Section {
                HStack(spacing: 12) {
                    GHAvatar(person: GHPerson(login: "octocat-acme", tint: .teal), size: 36)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(verbatim: "github.acme-corp.com").lineLimit(1)
                        GHLabel("octocat-acme from studio · gh", systemImage: "macstudio")
                            .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
            } footer: {
                Text(GHCopy.enterprise)
            }
        }
    }

    private var pullRequests: some View {
        GHTabs(selected: .workspaces) {
            NavigationStack {
                List {
                    Section {
                        VStack(alignment: .leading, spacing: 8) {
                            GHLabel("octo-org: public repositories only", systemImage: "exclamationmark.triangle.fill")
                                .font(.subheadline.weight(.semibold)).foregroundStyle(.orange)
                            Text("octo-org has not installed Sheprd's GitHub App. Its private pull requests come from your computers, marked “via MacBook Pro”.")
                                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                            HStack {
                                Link("Install Sheprd on octo-org", destination: installURL).font(.subheadline.weight(.semibold))
                                Spacer()
                                Button("Hide") {}.font(.subheadline).foregroundStyle(.secondary)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    Section("Mine") {
                        pull("sheprd-labs/sheprd #1412", GH.title, via: false)
                        pull("octo-org/infra #902", "Bump relay droplets to 2 vCPU", via: true)
                    }
                    Section("Review requested") {
                        pull("octo-org/platform #88", "Rotate deploy tokens weekly", via: true)
                        pull("sheprd-labs/sheprd #1398", "Teach the relay to drain slowly", via: false)
                    }
                }
                .ghScreen("Pull Requests")
            }
        }
    }

    private func pull(_ id: String, _ title: String, via: Bool) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: GHState.open.symbol).font(.caption.weight(.semibold)).foregroundStyle(GHState.open.color)
                Text(verbatim: id).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                Spacer(minLength: 4)
                if via { GHVia() }
            }
            Text(title).lineLimit(2)
        }
    }
}
