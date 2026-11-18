import Foundation
import Testing
@testable import SheprdGitHub
@testable import SheprdRemoteCore

struct GitHubHostTests {
    @Test(arguments: [
        ("github.com", "https://api.github.com", "https://api.github.com/graphql", "https://github.com"),
        ("GitHub.com.", "https://api.github.com", "https://api.github.com/graphql", "https://github.com"),
        ("api.github.com", "https://api.github.com", "https://api.github.com/graphql", "https://github.com"),
        ("acme.ghe.com", "https://api.acme.ghe.com", "https://api.acme.ghe.com/graphql", "https://acme.ghe.com"),
        ("github.example.com", "https://github.example.com/api/v3", "https://github.example.com/api/graphql", "https://github.example.com"),
        ("git.example.com:8443", "https://git.example.com:8443/api/v3", "https://git.example.com:8443/api/graphql", "https://git.example.com:8443"),
        // Not a GHE.com subdomain: a server of its own.
        ("a.b.ghe.com", "https://a.b.ghe.com/api/v3", "https://a.b.ghe.com/api/graphql", "https://a.b.ghe.com"),
    ])
    func eachKindOfHostHasItsOwnAPIs(host: String, rest: String, graphQL: String, web: String) throws {
        let parsed = try #require(GitHubHost(host))
        #expect(parsed.restBase.absoluteString == rest)
        #expect(parsed.graphQL.absoluteString == graphQL)
        #expect(parsed.web == web)
    }

    @Test(arguments: ["", "https://github.com", "github.com/acme", "user@github.com", "github.com?x", ".github.com", "a..b", "host:1:2", "gith ub.com"])
    func anythingButAHostNameIsRefused(text: String) {
        #expect(GitHubHost(text) == nil)
    }
}
