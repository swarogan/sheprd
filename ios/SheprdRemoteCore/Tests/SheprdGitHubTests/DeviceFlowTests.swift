import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct DeviceFlowTests {
    private let clock = TestClock()
    private let transport = FakeTransport()

    private func flow(sleep: GitHubDeviceFlow.Sleep? = nil) -> GitHubDeviceFlow {
        GitHubDeviceFlow(transport: transport.transport, now: clock.read, sleep: sleep ?? TestSleeper(clock: clock).sleep)
    }

    @Test func aCodeIsAskedForWithSheprdsAppAndRepoScope() async throws {
        transport.enqueue(SignInFixtures.code())
        let code = try await flow().requestCode()
        #expect(code.userCode == "WDJB-MJHT")
        #expect(code.verificationURI.absoluteString == "https://github.com/login/device")
        #expect(code.expires == GitHubFixtures.start.addingTimeInterval(900))
        #expect(code.interval == 5)
        let request = try #require(transport.requests.first)
        #expect(request.url?.absoluteString == "https://github.com/login/device/code")
        #expect(request.httpMethod == "POST")
        #expect(request.value(forHTTPHeaderField: "Accept") == "application/json")
        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)
        #expect(SignInFixtures.form(request) == ["client_id": "Iv23liurUcwxPjrdIFYT", "scope": "repo"])
    }

    @Test func aMissingIntervalIsFiveSeconds() async throws {
        transport.enqueue(SignInFixtures.json(#"{"device_code":"dc","user_code":"AB-12","verification_uri":"https://github.com/login/device","expires_in":60}"#))
        #expect(try await flow().requestCode().interval == 5)
    }

    @Test(arguments: [
        (SignInFixtures.code(userCode: ""), DeviceCodeField.userCode),
        (SignInFixtures.code(userCode: "ABCD EFGH"), .userCode),
        (SignInFixtures.code(userCode: String(repeating: "A", count: 33)), .userCode),
        (SignInFixtures.code(userCode: "ÄBCD-EFGH"), .userCode),
        (SignInFixtures.code(uri: "http://github.com/login/device"), .verificationURI),
        (SignInFixtures.code(uri: "https://evil.example/login/device"), .verificationURI),
        (SignInFixtures.code(uri: "https://github.com/login/device?next=x"), .verificationURI),
        (SignInFixtures.code(expiresIn: 0), .expiresIn),
        (SignInFixtures.code(expiresIn: 901), .expiresIn),
        (SignInFixtures.code(interval: 0), .interval),
        (SignInFixtures.code(interval: 901), .interval),
        (SignInFixtures.json(#"{"device_code":"has space","user_code":"AB","verification_uri":"https://github.com/login/device","expires_in":60}"#), .deviceCode),
    ])
    func aReplyFailingACheckIsRefused(reply: GitHubResponse, field: DeviceCodeField) async throws {
        transport.enqueue(reply)
        await #expect(throws: GitHubSignInError.invalidDeviceCode(field)) { try await flow().requestCode() }
    }

    @Test func anUnreadableOrRefusedCodeReplyIsTyped() async throws {
        transport.enqueue(SignInFixtures.json("not json"))
        transport.enqueue(GitHubResponse(status: 503))
        await #expect(throws: GitHubSignInError.malformed) { try await flow().requestCode() }
        await #expect(throws: GitHubSignInError.status(503)) { try await flow().requestCode() }
        await #expect(throws: GitHubSignInError.unreachable) { try await flow().requestCode() }
    }

    @Test func pollingWaitsAsGitHubAsksUntilTheToken() async throws {
        let sleeper = TestSleeper(clock: clock)
        let flow = flow(sleep: sleeper.sleep)
        transport.enqueue(SignInFixtures.code())
        let code = try await flow.requestCode()
        transport.enqueue(SignInFixtures.error("authorization_pending"))
        transport.enqueue(SignInFixtures.error("slow_down"))
        transport.enqueue(SignInFixtures.error("authorization_pending"))
        transport.enqueue(SignInFixtures.token("ghu_a1"))
        let credential = try await flow.poll(code)
        #expect(sleeper.recorded == [.seconds(5), .seconds(5), .seconds(10), .seconds(10)])
        #expect(credential.accessToken == GitHubToken("ghu_a1"))
        #expect(credential.refreshToken == GitHubToken("ghr_r1"))
        #expect(credential.expiresAt == clock.now.addingTimeInterval(28800))
        let poll = try #require(transport.requests.last)
        #expect(poll.url?.absoluteString == "https://github.com/login/oauth/access_token")
        #expect(SignInFixtures.form(poll) == [
            "client_id": "Iv23liurUcwxPjrdIFYT",
            "device_code": "3584d83530557fdd1f46af8289938c8ef79f9dc5",
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        ])
    }

    @Test(arguments: [
        ("expired_token", GitHubSignInError.expired),
        ("access_denied", .denied),
        ("unsupported_grant_type", .refused),
        ("bad_refresh_token", .refused),
    ])
    func aRefusalEndsPolling(code: String, error: GitHubSignInError) async throws {
        transport.enqueue(SignInFixtures.code())
        let flow = flow()
        let device = try await flow.requestCode()
        transport.enqueue(SignInFixtures.error(code))
        await #expect(throws: error) { try await flow.poll(device) }
    }

    @Test func theDeadlineEndsPollingWithoutAskingAgain() async throws {
        transport.enqueue(SignInFixtures.code(interval: 5, expiresIn: 8))
        let flow = flow()
        let device = try await flow.requestCode()
        transport.enqueue(SignInFixtures.error("authorization_pending"))
        await #expect(throws: GitHubSignInError.expired) { try await flow.poll(device) }
        #expect(transport.requests.count == 2)
    }

    @Test(arguments: [
        SignInFixtures.json(#"{"access_token":"ghu_a1","token_type":"mac"}"#),
        SignInFixtures.json(#"{"access_token":"","token_type":"bearer"}"#),
        SignInFixtures.json(#"{"token_type":"bearer"}"#),
    ])
    func anUnusableTokenIsRefused(reply: GitHubResponse) async throws {
        transport.enqueue(SignInFixtures.code())
        let flow = flow()
        let device = try await flow.requestCode()
        transport.enqueue(reply)
        await #expect(throws: GitHubSignInError.self) { try await flow.poll(device) }
    }

    @Test func cancellingStopsPolling() async throws {
        transport.enqueue(SignInFixtures.code())
        let flow = GitHubDeviceFlow(transport: transport.transport, now: clock.read) { try await Task.sleep(for: .seconds($0.components.seconds * 1000)) }
        let device = try await flow.requestCode()
        let polling = Task { try await flow.poll(device) }
        polling.cancel()
        await #expect(throws: CancellationError.self) { try await polling.value }
        #expect(transport.requests.count == 1)
    }

    @Test func refreshingSendsNoSecretAndKeepsTheLogin() async throws {
        transport.enqueue(SignInFixtures.token("ghu_a2", refresh: "ghr_r2"))
        let renewed = try await flow().refresh(SignInFixtures.credential())
        #expect(renewed.accessToken == GitHubToken("ghu_a2"))
        #expect(renewed.refreshToken == GitHubToken("ghr_r2"))
        #expect(renewed.login == "ana")
        let request = try #require(transport.requests.first)
        #expect(SignInFixtures.form(request) == ["client_id": "Iv23liurUcwxPjrdIFYT", "grant_type": "refresh_token", "refresh_token": "ghr_r1"])
    }

    @Test func aRevokedRefreshTokenIsTyped() async throws {
        transport.enqueue(SignInFixtures.error("bad_refresh_token"))
        await #expect(throws: GitHubSignInError.refreshRejected) { try await flow().refresh(SignInFixtures.credential()) }
    }
}
