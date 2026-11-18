import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// The phone's own sign-in comes first for github.com, then a lent token,
/// then the computer.
struct SignInPrecedenceTests {
    private let transport = FakeTransport()
    private let lenders = FakeLenders()
    private let vault = SecretVault.memory()
    private var store: GitHubSignInStore { GitHubSignInStore(vault: vault) }

    private func reader(signIn transport: GitHubTransport? = nil) -> PullRequestReader {
        let now: @Sendable () -> Date = { GitHubFixtures.start }
        return PullRequestReader(
            leases: GitHubLeases(store: TokenStore(vault: .memory()), now: now),
            api: GitHubAPI(transport: self.transport.transport, now: now),
            signIn: GitHubSignIn(flow: GitHubDeviceFlow(transport: transport ?? self.transport.transport, now: now), store: store)
        )
    }

    private var source: Lenders { Lenders(lenders: [GitHubFixtures.lender("mac", hosts: ["github.com", "acme.ghe.com"])], fetch: lenders.fetch) }
    private let computers = PullRequestChecks(number: 7, checks: [])

    private var authorizations: [String?] { transport.requests.map { $0.value(forHTTPHeaderField: "Authorization") } }

    @Test func theOwnSignInWins() async throws {
        try store.save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(authorizations == ["Bearer ghu_a1"])
        #expect(lenders.asked.isEmpty)
    }

    @Test func signedOutUsesALentToken() async throws {
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(authorizations == ["Bearer lent"])
    }

    @Test(arguments: [GitHubResponse(status: 404), GitHubResponse(status: 403)])
    func aRepositoryTheOwnSignInCannotReadFallsBackToALentToken(answer: GitHubResponse) async throws {
        try store.save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        transport.enqueue(answer)
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(read.value.number == 42)
        #expect(authorizations == ["Bearer ghu_a1", "Bearer lent"])
    }

    @Test func aRefusedOwnTokenIsRenewedThenFallsBack() async throws {
        try store.save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        let oauth = FakeTransport([SignInFixtures.token("ghu_a2", refresh: "ghr_r2")])
        transport.enqueue(GitHubResponse(status: 401))
        transport.enqueue(GitHubResponse(status: 401))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let read = try await reader(signIn: oauth.transport).checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(authorizations == ["Bearer ghu_a1", "Bearer ghu_a2", "Bearer lent"])
        #expect(oauth.requests.count == 1)
        // The rotated pair stays signed in.
        #expect(try store.load()?.accessToken == GitHubToken("ghu_a2"))
    }

    @Test func aRevokedOwnSignInFallsBackAndSignsOut() async throws {
        try store.save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("lent"))
        let oauth = FakeTransport([SignInFixtures.error("bad_refresh_token")])
        transport.enqueue(GitHubResponse(status: 401))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let reader = reader(signIn: oauth.transport)
        let read = try await reader.checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(authorizations == ["Bearer ghu_a1", "Bearer lent"])
        #expect(try store.load() == nil)
        #expect(await reader.signIn?.state == .failed(.refreshRejected))
    }

    @Test func everythingFailingAsksTheComputer() async throws {
        try store.save(SignInFixtures.credential())
        transport.enqueue(GitHubResponse(status: 404))
        let read = try await reader().checks(workspace: GitHubFixtures.workspace(), computer: "Mac", source: source) { computers }
        #expect(read.origin == .computer("Mac"))
        #expect(lenders.asked == ["mac"])
    }

    @Test func enterpriseHostsUseLentTokensOnly() async throws {
        try store.save(SignInFixtures.credential())
        lenders.answer("mac", GitHubFixtures.answer("ghe", host: "acme.ghe.com", source: .gh))
        transport.enqueue(try GitHubFixtures.response("open_pr"))
        let workspace = GitHubFixtures.workspace(github: .init(host: "acme.ghe.com", owner: "acme", repo: "app", branch: "feature"))
        let read = try await reader().checks(workspace: workspace, computer: "Mac", source: source) { computers }
        #expect(read.origin == .github)
        #expect(authorizations == ["Bearer ghe"])
        #expect(transport.requests.first?.url?.host() == "api.acme.ghe.com")
    }
}
