import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

/// The organizations listed after signing in, and what the App reads in
/// each.
struct GitHubOrganizationsTests {
    private let installations = SignInFixtures.json(#"""
    {"total_count":3,"installations":[
      {"id":1,"repository_selection":"all","account":{"login":"sheprd-labs","type":"Organization"}},
      {"id":2,"repository_selection":"selected","account":{"login":"Acme","type":"Organization"}},
      {"id":3,"repository_selection":"all","account":{"login":"octocat","type":"User"}}
    ]}
    """#)
    private let orgs = SignInFixtures.json(#"[{"login":"octo-org","id":9},{"login":"acme","id":8},{"login":"bad/name","id":7}]"#)

    @Test func installedAndNotInstalledOrganizations() async throws {
        let transport = FakeTransport([installations, orgs])
        let found = await GitHubOrganization.read(token: GitHubToken("ghu_a1"), transport: transport.transport)
        #expect(found == [
            GitHubOrganization(login: "Acme", access: .selected),
            GitHubOrganization(login: "octo-org", access: .publicOnly),
            GitHubOrganization(login: "sheprd-labs", access: .all),
        ])
        #expect(transport.requests.map { $0.url?.path() } == ["/user/installations", "/user/orgs"])
    }

    @Test func organizationsThatCannotBeListedLeaveTheInstalledOnes() async throws {
        let transport = FakeTransport([installations, GitHubResponse(status: 403)])
        let found = await GitHubOrganization.read(token: GitHubToken("ghu_a1"), transport: transport.transport)
        #expect(found?.map(\.login) == ["Acme", "sheprd-labs"])
    }

    @Test func noInstallationsReadIsUnknown() async throws {
        let transport = FakeTransport([GitHubResponse(status: 500)])
        #expect(await GitHubOrganization.read(token: GitHubToken("ghu_a1"), transport: transport.transport) == nil)
    }

    @Test func onlyAppUserTokensAreChecked() async throws {
        let transport = FakeTransport()
        #expect(await GitHubOrganization.read(token: GitHubToken("gho_x"), transport: transport.transport) == nil)
        #expect(transport.requests.isEmpty)
    }
}
