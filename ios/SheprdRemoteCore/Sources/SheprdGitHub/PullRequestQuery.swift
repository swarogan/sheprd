import Foundation

/// One GraphQL query for a repository's branches: each branch an aliased
/// `pullRequests` field, so the workspaces on screen cost one request per
/// repository. Names travel as variables, never inside the query text.
struct PullRequestQuery: Sendable, Equatable {
    /// Branches one query asks for at most: each reads a few hundred
    /// nodes, so twenty stay well inside GitHub's node limit, as on the
    /// computer (`pull_request/batch.rs`).
    static let batchLimit = 20
    /// Pull requests read per branch: a branch name reused after its first
    /// pull request closed lists both.
    static let perBranch = 5

    var owner: String
    var repo: String
    /// The head branches asked for, in alias order (`b0`, `b1`, ...).
    var branches: [String]

    init(owner: String, repo: String, branches: [String]) {
        self.owner = owner
        self.repo = repo
        self.branches = branches
    }

    /// `branches` without repeats, in queries of at most `batchLimit`.
    static func batches(owner: String, repo: String, branches: [String]) -> [PullRequestQuery] {
        var seen = Set<String>()
        let unique = branches.filter { seen.insert($0).inserted }
        return stride(from: 0, to: unique.count, by: batchLimit).map { start in
            PullRequestQuery(owner: owner, repo: repo, branches: Array(unique[start..<min(start + batchLimit, unique.count)]))
        }
    }

    static func alias(_ index: Int) -> String { "b\(index)" }

    /// The query's text.
    var text: String {
        let parameters = (["$owner: String!", "$repo: String!"] + branches.indices.map { "$\(Self.alias($0)): String!" })
            .joined(separator: ", ")
        let fields = branches.indices.map { index in
            let alias = Self.alias(index)
            return "    \(alias): pullRequests(first: \(Self.perBranch), headRefName: $\(alias), orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { ...PullRequestFields } }"
        }
        return """
        query(\(parameters)) {
          repository(owner: $owner, name: $repo) {
            nameWithOwner
        \(fields.joined(separator: "\n"))
          }
        }
        \(Self.fragment)
        """
    }

    /// The JSON body of the request.
    var body: Data {
        get throws {
            var variables = ["owner": owner, "repo": repo]
            for (index, branch) in branches.enumerated() { variables[Self.alias(index)] = branch }
            return try JSONSerialization.data(withJSONObject: ["query": text, "variables": variables], options: [.sortedKeys])
        }
    }

    static let fragment = """
    fragment PullRequestFields on PullRequest {
      number title url state isDraft reviewDecision mergeable mergeStateStatus headRefName headRefOid
      author { login }
      headRepositoryOwner { login }
      commits(last: 1) { nodes { commit { statusCheckRollup {
        contexts(first: 100) {
          pageInfo { hasNextPage }
          nodes { __typename ... on CheckRun { name status conclusion detailsUrl databaseId } ... on StatusContext { context state targetUrl } }
        }
      } } } }
    }
    """
}
