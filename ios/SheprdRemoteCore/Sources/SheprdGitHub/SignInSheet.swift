import Foundation

/// What the sign-in sheet shows, from the sign-in's states: the code to
/// copy, then waiting once GitHub was opened, until it expires, GitHub
/// denies it, or the account is signed in. It follows only the sign-in it
/// started: a state from before (a sign-in already held, an earlier
/// failure) shows nothing until GitHub is asked for a code.
public struct GitHubSignInSheet: Equatable, Sendable {
    public enum Step: Equatable, Sendable {
        /// Asking GitHub for a code.
        case requesting
        /// The code, ready to copy.
        case code(GitHubDeviceCode)
        /// GitHub was opened; polling until the person approves.
        case waiting(GitHubDeviceCode)
        case expired
        case denied
        case failed(GitHubSignInError)
        case signedIn(login: String, access: GitHubAppAccess)
        /// Stopped without signing in.
        case cancelled

        /// The code shown, while it can still be used.
        public var code: GitHubDeviceCode? {
            switch self {
            case let .code(code), let .waiting(code): code
            default: nil
            }
        }

        /// Asking again is what helps: a new code, or another try.
        public var retryable: Bool {
            switch self {
            case .expired, .denied, .failed: true
            default: false
            }
        }
    }

    public private(set) var step = Step.requesting
    /// The code last shown, kept struck through once it expired or was
    /// denied.
    public private(set) var lastCode: String?
    /// A state of this sheet's own sign-in has arrived.
    private var started = false
    private var opened = false

    public init() {}

    /// A new code is asked for: on opening, and on Get a New Code or Try
    /// Again.
    public mutating func begin() {
        step = .requesting
        lastCode = nil
        started = false
        opened = false
    }

    public mutating func receive(_ state: GitHubSignInState, now: Date) {
        switch state {
        case .requesting:
            started = true
            step = .requesting
        case let .waiting(code):
            started = true
            lastCode = code.userCode
            step = now >= code.expires ? .expired : opened ? .waiting(code) : .code(code)
        case let .failed(error) where started:
            step = switch error {
            case .expired: .expired
            case .denied: .denied
            default: .failed(error)
            }
        case let .signedIn(login, access) where started:
            step = .signedIn(login: login, access: access)
        case .idle where started:
            step = .cancelled
        case .failed, .signedIn, .idle:
            break
        }
    }

    /// GitHub was opened to enter the code.
    public mutating func openedGitHub() {
        opened = true
        if case let .code(code) = step { step = .waiting(code) }
    }

    /// The code expires on screen as its time runs out, before the next
    /// poll says so.
    public mutating func tick(now: Date) {
        if let code = step.code, now >= code.expires { step = .expired }
    }

    /// What is left of the code's time, as `mm:ss`.
    public func remaining(now: Date) -> String? {
        guard let code = step.code else { return nil }
        let seconds = max(0, Int(code.expires.timeIntervalSince(now).rounded(.up)))
        return String(format: "%d:%02d", seconds / 60, seconds % 60)
    }
}

#if DEBUG
extension GitHubDeviceCode {
    /// A code for screenshots and previews; it signs nothing in.
    public static func sample(userCode: String = "WDJB-MJHT", expires: Date) -> GitHubDeviceCode {
        GitHubDeviceCode(
            deviceCode: GitHubToken("sample"), userCode: userCode,
            verificationURI: URL(string: GitHubDeviceFlow.verificationURI) ?? URL(fileURLWithPath: "/"), expires: expires, interval: 5
        )
    }
}
#endif
