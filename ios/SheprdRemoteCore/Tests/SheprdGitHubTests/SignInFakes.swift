import Foundation
import Synchronization
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// Fixtures for the phone's own sign-in: GitHub's OAuth answers and a
/// sleep that only moves the test clock.
enum SignInFixtures {
    static func json(_ text: String, status: Int = 200) -> GitHubResponse {
        GitHubResponse(status: status, headers: ["Content-Type": "application/json"], body: Data(text.utf8))
    }

    static func code(interval: Int = 5, expiresIn: Int = 900, userCode: String = "WDJB-MJHT", uri: String = "https://github.com/login/device") -> GitHubResponse {
        json(#"{"device_code":"3584d83530557fdd1f46af8289938c8ef79f9dc5","user_code":"\#(userCode)","verification_uri":"\#(uri)","expires_in":\#(expiresIn),"interval":\#(interval)}"#)
    }

    static func error(_ code: String) -> GitHubResponse {
        json(#"{"error":"\#(code)","error_description":"echoed"}"#)
    }

    static func token(_ access: String, refresh: String? = "ghr_r1", expiresIn: Int? = 28800) -> GitHubResponse {
        var fields = [#""access_token":"\#(access)""#, #""token_type":"bearer""#, #""scope":"""#]
        if let refresh { fields.append(#""refresh_token":"\#(refresh)""#) }
        if let expiresIn { fields.append(#""expires_in":\#(expiresIn)"#) }
        return json("{\(fields.joined(separator: ","))}")
    }

    static func user(_ login: String = "ana") -> GitHubResponse {
        json(#"{"login":"\#(login)","id":1}"#)
    }

    static func installations(_ count: Int) -> GitHubResponse {
        let items = (0..<count).map { #"{"id":\#($0 + 1)}"# }.joined(separator: ",")
        return json(#"{"total_count":\#(count),"installations":[\#(items)]}"#)
    }

    static func credential(_ access: String = "ghu_a1", refresh: String? = "ghr_r1", expiresIn: TimeInterval? = 8 * 3600, login: String = "ana") -> GitHubSignInCredential {
        GitHubSignInCredential(
            accessToken: GitHubToken(access), refreshToken: refresh.map(GitHubToken.init), clientID: GitHubDeviceFlow.clientID,
            expiresAt: expiresIn.map { GitHubFixtures.start.addingTimeInterval($0) }, login: login
        )
    }

    /// The form fields a request sent.
    static func form(_ request: URLRequest) -> [String: String] {
        let text = String(decoding: request.httpBody ?? Data(), as: UTF8.self)
        var fields: [String: String] = [:]
        for pair in text.split(separator: "&") {
            let parts = pair.split(separator: "=", maxSplits: 1).map { String($0).removingPercentEncoding ?? "" }
            if parts.count == 2 { fields[parts[0]] = parts[1] }
        }
        return fields
    }
}

/// A sleep that records each wait and moves `clock` by it at once.
final class TestSleeper: Sendable {
    private let waits = Mutex<[Duration]>([])
    let clock: TestClock

    init(clock: TestClock) {
        self.clock = clock
    }

    var recorded: [Duration] { waits.withLock { $0 } }

    var sleep: GitHubDeviceFlow.Sleep {
        { [self] duration in
            try Task.checkCancellation()
            waits.withLock { $0.append(duration) }
            clock.advance(TimeInterval(duration.components.seconds))
        }
    }
}
