import AuthenticationServices
import SheprdGitHub
import SheprdRemoteCore
import SwiftUI
import UIKit

/// Signing in to github.com on this phone with GitHub's device flow: the
/// code to copy, GitHub's device page to enter it, waiting while the phone
/// polls, and then the account and what Sheprd's GitHub App reads in each
/// of its organizations.
struct GitHubSignInSheetView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var sheet = GitHubSignInSheet()
    @State private var page = DeviceLoginPage()
    @State private var organizations: [GitHubOrganization]?
    @State private var now = Date.now
    @State private var copied = 0
    /// A sample step on screen, not a sign-in.
    @State private var sample = false

    var body: some View {
        NavigationStack {
            content
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button(signedIn ? "Done" : "Cancel") { close() }
                    }
                }
                .navigationTitle(signedIn ? "Signed In" : "Sign In to GitHub")
                .navigationBarTitleDisplayMode(.inline)
        }
        .presentationDetents([.large])
        .task { await run() }
        .task(id: sheet.step.code?.userCode) { await countDown() }
        .onDisappear {
            page.close()
            if !signedIn, !sample { Task { await GitHubReads.reader.signIn?.cancel() } }
        }
        .sensoryFeedback(.success, trigger: copied)
    }

    private var signedIn: Bool {
        if case .signedIn = sheet.step { true } else { false }
    }

    @ViewBuilder private var content: some View {
        switch sheet.step {
        case let .signedIn(login, access):
            SignedInList(login: login, access: access, organizations: organizations)
        default:
            ScrollView {
                VStack(spacing: 18) {
                    Image(systemName: problem?.symbol ?? "person.badge.key.fill")
                        .font(.system(size: 44))
                        .foregroundStyle(problem == nil ? AnyShapeStyle(.tint) : AnyShapeStyle(.red))
                        .padding(.top, 12)
                        .accessibilityHidden(true)
                    Text(problem?.title ?? "Enter this code on GitHub")
                        .font(.title2.weight(.semibold)).multilineTextAlignment(.center)
                    Text(problem?.detail ?? "Copy it, then open GitHub and paste it. This iPhone then reads your pull requests and checks from GitHub.")
                        .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                    codeBox
                    status
                    actions
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
            }
        }
    }

    /// What went wrong, when something did.
    private var problem: (title: String, detail: String, symbol: String)? {
        let device = UIDevice.current.model
        switch sheet.step {
        case .expired:
            return ("This code expired", "Codes last 15 minutes. Get a new one and enter it on GitHub.", "clock.badge.xmark")
        case .denied:
            return ("GitHub denied the sign-in", "Cancel was chosen on GitHub, or an owner refused the request. Nothing was saved on this \(device).", "xmark.octagon")
        case let .failed(error):
            return ("Could not sign in", Self.explain(error, device: device), "exclamationmark.triangle")
        default:
            return nil
        }
    }

    private static func explain(_ error: GitHubSignInError, device: String) -> String {
        switch error {
        case .unreachable: "GitHub could not be reached. Check the connection and try again."
        case .refused: "GitHub refused the sign-in. An organization policy may not allow it."
        case .storage: "The sign-in could not be kept in this \(device)'s Keychain."
        default: "GitHub answered in a way this \(device) did not expect. Try again."
        }
    }

    /// The code in large letters, with its copy button.
    private var codeBox: some View {
        let failed = sheet.step.retryable
        let code = sheet.step.code?.userCode
        return HStack(spacing: 10) {
            Group {
                if let shown = code ?? (failed ? sheet.lastCode : nil) {
                    Text(verbatim: shown)
                } else if failed {
                    Text(verbatim: "—")
                } else {
                    ProgressView().frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .font(.system(.largeTitle, design: .monospaced).weight(.semibold))
            .tracking(3)
            .strikethrough(sheet.step == .expired)
            .foregroundStyle(failed ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.primary))
            .lineLimit(1).minimumScaleFactor(0.6)
            .textSelection(.enabled)
            Spacer(minLength: 0)
            Button("Copy", systemImage: "doc.on.doc") {
                guard let code else { return }
                UIPasteboard.general.string = code
                copied += 1
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.bordered)
            .disabled(code == nil)
        }
        .padding(.horizontal, 18).padding(.vertical, 14)
        .background(.fill.tertiary, in: .rect(cornerRadius: 16))
    }

    @ViewBuilder private var status: some View {
        if case .waiting = sheet.step {
            HStack(spacing: 10) {
                ProgressView()
                Text("Waiting for you to approve on GitHub").font(.subheadline)
                Spacer(minLength: 4)
                if let left = sheet.remaining(now: now) {
                    Text("Expires in \(left)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
            }
            .padding(12)
            .background(.fill.quaternary, in: .rect(cornerRadius: 12))
        }
    }

    @ViewBuilder private var actions: some View {
        if sheet.step.retryable {
            Button {
                Task { await start() }
            } label: {
                Text(sheet.step == .expired ? "Get a New Code" : "Try Again").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent).controlSize(.large)
        } else {
            let waiting = if case .waiting = sheet.step { true } else { false }
            VStack(spacing: 6) {
                Group {
                    if waiting {
                        openButton.buttonStyle(.bordered)
                    } else {
                        openButton.buttonStyle(.borderedProminent)
                    }
                }
                .controlSize(.large)
                .disabled(sheet.step.code == nil)
                Text(verbatim: "github.com/login/device").font(.caption.monospaced()).foregroundStyle(.secondary)
            }
        }
    }

    private var openButton: some View {
        Button {
            guard let code = sheet.step.code else { return }
            // The code is on the pasteboard, ready to paste on GitHub's page.
            UIPasteboard.general.string = code.userCode
            page.open(code.verificationURI)
            sheet.openedGitHub()
        } label: {
            Label("Open GitHub", systemImage: "safari").frame(maxWidth: .infinity)
        }
    }

    // MARK: Running the sign-in

    private func run() async {
        #if DEBUG
        if let step = UserDefaults.standard.string(forKey: "gitHubSignIn"), showSample(step) { return }
        #endif
        guard let signIn = await GitHubReads.reader.signIn else { return }
        // Watch first, so no state of this sign-in is missed.
        let states = await signIn.states()
        await start()
        for await state in states {
            sheet.receive(state, now: .now)
            if case .signedIn = sheet.step {
                page.close()
                if organizations == nil, let token = try? await signIn.token() {
                    organizations = await GitHubOrganization.read(token: token, transport: GitHubHTTP.urlSession())
                }
            }
        }
    }

    private func start() async {
        sheet.begin()
        organizations = nil
        // Failures arrive as states.
        _ = try? await GitHubReads.reader.signIn?.start()
    }

    /// Keeps "Expires in" current, and expires the code on screen.
    private func countDown() async {
        while !Task.isCancelled, sheet.step.code != nil {
            now = .now
            if !sample { sheet.tick(now: now) }
            try? await Task.sleep(for: .seconds(1))
        }
    }

    private func close() {
        page.close()
        if !signedIn, !sample { Task { await GitHubReads.reader.signIn?.cancel() } }
        dismiss()
    }

    #if DEBUG
    /// `-gitHubSignIn code|waiting|expired|denied|done` shows that step
    /// with sample data, for screenshots.
    private func showSample(_ step: String) -> Bool {
        let code = GitHubDeviceCode.sample(expires: .now.addingTimeInterval(872))
        sample = true
        sheet.begin()
        sheet.receive(.waiting(code), now: .now)
        switch step {
        case "code": break
        case "waiting": sheet.openedGitHub()
        case "expired": sheet.receive(.failed(.expired), now: .now)
        case "denied": sheet.receive(.failed(.denied), now: .now)
        case "done":
            sheet.receive(.signedIn(login: "octocat", access: .installed), now: .now)
            organizations = GitHubSample.organizations
        default:
            sample = false
            return false
        }
        return true
    }
    #endif
}

/// The account signed in, and what the App reads in its organizations.
private struct SignedInList: View {
    var login: String
    var access: GitHubAppAccess
    var organizations: [GitHubOrganization]?

    var body: some View {
        List {
            Section {
                VStack(spacing: 6) {
                    PullRequestAvatar(login: login, size: 64)
                    Text(verbatim: login).font(.title3.weight(.semibold))
                    IconText("Signed in on this \(UIDevice.current.model)", systemImage: "checkmark.seal.fill")
                        .font(.subheadline).foregroundStyle(.green)
                }
                .frame(maxWidth: .infinity)
                .listRowBackground(Color.clear)
            }
            if let organizations, !organizations.isEmpty {
                Section {
                    ForEach(organizations) { OrganizationRow(organization: $0) }
                    ForEach(organizations.filter { $0.access == .publicOnly }) { organization in
                        if let url = URL(string: GitHubDeviceFlow.installURL) {
                            Link(destination: url) {
                                Label("Install Sheprd on \(organization.login)", systemImage: "arrow.up.forward.square")
                            }
                        }
                    }
                } header: {
                    Text("Organizations")
                } footer: {
                    if let first = organizations.first(where: { $0.access == .publicOnly }) {
                        Text("\(first.login) has not installed Sheprd's GitHub App, so this \(UIDevice.current.model) reads only its public repositories. An owner can install it; until then, its private pull requests come from your computers.")
                    }
                }
            } else if access == .notInstalled {
                Section {
                    if let url = URL(string: GitHubDeviceFlow.installURL) {
                        Link(destination: url) { Label("Install Sheprd on GitHub", systemImage: "arrow.up.forward.square") }
                    }
                } footer: {
                    Text("Sheprd's GitHub App is not installed on any of your accounts, so this \(UIDevice.current.model) reads only public repositories. Until it is, private pull requests come from your computers.")
                }
            }
        }
    }
}

private struct OrganizationRow: View {
    var organization: GitHubOrganization

    var body: some View {
        HStack(spacing: 12) {
            PullRequestAvatar(login: organization.login, size: 30)
            Text(verbatim: organization.login).lineLimit(1)
            Spacer(minLength: 6)
            access.font(.caption).lineLimit(1)
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder private var access: some View {
        switch organization.access {
        case .all: IconText("All repositories", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
        case .selected: IconText("Selected repositories", systemImage: "checkmark.circle").foregroundStyle(.green)
        case .publicOnly: IconText("Public repositories only", systemImage: "exclamationmark.triangle.fill").foregroundStyle(.orange)
        }
    }
}

/// GitHub's device page in an in-app browser session. The device flow has
/// no redirect back, so the session never completes on its own: it shares
/// Safari's cookies (an account already signed in on GitHub stays signed
/// in), keeps the app in front so polling goes on, and is closed as soon
/// as the sign-in completes. If it cannot start, Safari opens the page.
@MainActor
final class DeviceLoginPage {
    private var session: ASWebAuthenticationSession?
    /// The session holds its provider weakly.
    private var anchor: Anchor?

    func open(_ url: URL) {
        close()
        let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
        guard let window = windows.first(where: \.isKeyWindow) ?? windows.first else {
            UIApplication.shared.open(url)
            return
        }
        // Never called back: GitHub's device page redirects nowhere.
        let session = ASWebAuthenticationSession(url: url, callback: .customScheme("sheprd-github-device")) { _, _ in }
        let anchor = Anchor(window: window)
        session.presentationContextProvider = anchor
        session.prefersEphemeralWebBrowserSession = false
        self.session = session
        self.anchor = anchor
        if !session.start() {
            close()
            UIApplication.shared.open(url)
        }
    }

    func close() {
        session?.cancel()
        session = nil
        anchor = nil
    }

    private final class Anchor: NSObject, ASWebAuthenticationPresentationContextProviding {
        let window: UIWindow

        init(window: UIWindow) {
            self.window = window
        }

        func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
            window
        }
    }
}
