import Foundation
import Synchronization
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// Shared fixtures for the GitHub tests: recorded answers, a transport that
/// replays them, and computers that lend tokens.
enum GitHubFixtures {
    static let host = GitHubHost("github.com")!
    static let start = Date(timeIntervalSince1970: 1_800_000_000)

    static func data(_ name: String) throws -> Data {
        let url = try #require(Bundle.module.url(forResource: "Fixtures/github/\(name)", withExtension: "json"))
        return try Data(contentsOf: url)
    }

    static func response(_ name: String, status: Int = 200, headers: [String: String] = [:]) throws -> GitHubResponse {
        GitHubResponse(status: status, headers: headers, body: try data(name))
    }

    static func workspace(id: String = "w", github: Home.GitHubRepository? = .init(host: "github.com", owner: "acme", repo: "app", branch: "feature")) -> Home.Workspace {
        Home.Workspace(id: id, label: id, branch: github?.branch, cwd: "/\(id)", pullRequest: nil, chats: [], terminals: [], github: github)
    }

    static func lender(_ device: String, reached: TimeInterval? = 0, hosts: [String] = ["github.com"]) -> GitHubLender {
        GitHubLender(device: device, name: device.capitalized, hosts: Set(hosts.compactMap(GitHubHost.init)), reached: reached.map { start.addingTimeInterval($0) })
    }

    static func answer(_ token: String, expiresIn: TimeInterval? = nil, host: String = "github.com", source: LeaseSource = .sheprd) -> LeaseFetch {
        .lease(LeaseAnswer(host: host, login: "ana", token: GitHubToken(token), expiresAt: expiresIn.map { Int64(start.timeIntervalSince1970 + $0) }, source: source))
    }
}

/// A clock tests move by hand.
final class TestClock: Sendable {
    private let time = Mutex(GitHubFixtures.start)
    var now: Date { time.withLock { $0 } }
    func advance(_ seconds: TimeInterval) { time.withLock { $0.addTimeInterval(seconds) } }
    var read: @Sendable () -> Date { { [self] in now } }
}

/// Answers requests from a queue, and keeps them for the test to check.
final class FakeTransport: Sendable {
    private let state = Mutex<(answers: [GitHubResponse], requests: [URLRequest])>(([], []))

    init(_ answers: [GitHubResponse] = []) {
        state.withLock { $0.answers = answers }
    }

    func enqueue(_ answer: GitHubResponse) {
        state.withLock { $0.answers.append(answer) }
    }

    var requests: [URLRequest] { state.withLock { $0.requests } }

    var transport: GitHubTransport {
        { [self] request, _ in
            try state.withLock { state in
                state.requests.append(request)
                guard !state.answers.isEmpty else { throw URLError(.notConnectedToInternet) }
                return state.answers.removeFirst()
            }
        }
    }
}

/// Computers answering lease asks from a script, counting asks.
final class FakeLenders: Sendable {
    private let state = Mutex<(answers: [String: [LeaseFetch]], asked: [String])>(([:], []))

    func answer(_ device: String, _ answers: LeaseFetch...) {
        state.withLock { $0.answers[device, default: []].append(contentsOf: answers) }
    }

    var asked: [String] { state.withLock { $0.asked } }

    var fetch: FetchLease {
        { [self] device, _ in
            try state.withLock { state in
                state.asked.append(device)
                guard var queue = state.answers[device], !queue.isEmpty else { throw URLError(.cannotConnectToHost) }
                let answer = queue.removeFirst()
                state.answers[device] = queue
                return answer
            }
        }
    }
}
