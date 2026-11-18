import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// The sign-in sheet's steps, from the sign-in's states.
struct SignInSheetTests {
    private let now = GitHubFixtures.start
    private var code: GitHubDeviceCode {
        GitHubDeviceCode(
            deviceCode: GitHubToken("dc"), userCode: "WDJB-MJHT", verificationURI: URL(string: GitHubDeviceFlow.verificationURI)!,
            expires: now.addingTimeInterval(900), interval: 5
        )
    }

    private func sheet(_ states: GitHubSignInState...) -> GitHubSignInSheet {
        var sheet = GitHubSignInSheet()
        sheet.begin()
        for state in states { sheet.receive(state, now: now) }
        return sheet
    }

    @Test func theCodeShowsThenWaitsOnceGitHubIsOpened() {
        var sheet = sheet(.requesting, .waiting(code))
        #expect(sheet.step == .code(code))
        sheet.openedGitHub()
        #expect(sheet.step == .waiting(code))
        // Later states keep it waiting.
        sheet.receive(.waiting(code), now: now)
        #expect(sheet.step == .waiting(code))
    }

    @Test func aSignInHeldOrAnOldFailureShowsNothingUntilTheCodeIsAsked() {
        #expect(sheet(.signedIn(login: "old", access: .installed)).step == .requesting)
        #expect(sheet(.failed(.denied)).step == .requesting)
        #expect(sheet(.idle).step == .requesting)
    }

    @Test(arguments: [
        (GitHubSignInError.expired, GitHubSignInSheet.Step.expired),
        (.denied, .denied),
        (.unreachable, .failed(.unreachable)),
    ])
    func failuresAfterTheCode(error: GitHubSignInError, step: GitHubSignInSheet.Step) {
        let sheet = sheet(.requesting, .waiting(code), .failed(error))
        #expect(sheet.step == step)
        #expect(sheet.lastCode == "WDJB-MJHT")
        #expect(sheet.step.retryable)
    }

    @Test func aCodeThatCouldNotBeAskedForFails() {
        #expect(sheet(.requesting, .failed(.unreachable)).step == .failed(.unreachable))
    }

    @Test func signingInEndsOnTheAccount() {
        let sheet = sheet(.requesting, .waiting(code), .signedIn(login: "octocat", access: .unknown), .signedIn(login: "octocat", access: .notInstalled))
        #expect(sheet.step == .signedIn(login: "octocat", access: .notInstalled))
        #expect(!sheet.step.retryable)
    }

    @Test func cancellingElsewhereEndsTheSheet() {
        #expect(sheet(.requesting, .waiting(code), .idle).step == .cancelled)
    }

    @Test func theCodeExpiresOnScreen() {
        var sheet = sheet(.requesting, .waiting(code))
        #expect(sheet.remaining(now: now.addingTimeInterval(28)) == "14:32")
        sheet.tick(now: now.addingTimeInterval(899))
        #expect(sheet.step == .code(code))
        sheet.tick(now: now.addingTimeInterval(900))
        #expect(sheet.step == .expired)
        #expect(sheet.remaining(now: now) == nil)
    }

    @Test func anExpiredCodeArrivingIsExpired() {
        var sheet = GitHubSignInSheet()
        sheet.receive(.requesting, now: now)
        sheet.receive(.waiting(code), now: now.addingTimeInterval(1000))
        #expect(sheet.step == .expired)
    }

    @Test func beginningAgainForgetsTheLastTry() {
        var sheet = sheet(.requesting, .waiting(code), .failed(.expired))
        sheet.openedGitHub()
        sheet.begin()
        #expect(sheet.step == .requesting)
        #expect(sheet.lastCode == nil)
        sheet.receive(.failed(.expired), now: now)
        #expect(sheet.step == .requesting)
        sheet.receive(.waiting(code), now: now)
        #expect(sheet.step == .code(code))
    }

    @Test func theSheetFollowsARealSignIn() async throws {
        let clock = TestClock()
        let sleeper = TestSleeper(clock: clock)
        let transport = FakeTransport([
            SignInFixtures.code(), SignInFixtures.error("authorization_pending"), SignInFixtures.token("ghu_a1"),
            SignInFixtures.user("octocat"), SignInFixtures.installations(0),
        ])
        let signIn = GitHubSignIn(
            flow: GitHubDeviceFlow(transport: transport.transport, now: clock.read, sleep: sleeper.sleep),
            store: GitHubSignInStore(vault: .memory())
        )
        var sheet = GitHubSignInSheet()
        sheet.begin()
        let states = await signIn.states()
        _ = try await signIn.start()
        for await state in states {
            sheet.receive(state, now: clock.now)
            if case .signedIn(_, .notInstalled) = sheet.step { break }
            if sheet.step.retryable { break }
        }
        #expect(sheet.step == .signedIn(login: "octocat", access: .notInstalled))
    }
}
