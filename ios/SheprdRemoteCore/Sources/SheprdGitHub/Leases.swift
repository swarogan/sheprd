import Foundation

/// Why no lease could be had for a host; each means asking the computer
/// for the read instead.
public enum LeaseError: Error, Equatable, Sendable {
    /// No paired computer lends this host.
    case noLender
    /// Every computer that lends it refused: no account there any more
    /// (404), or sharing turned off (403).
    case refused
    /// A computer said to wait (429), or Sheprd there did not answer
    /// (504); no computer is asked for this host until then.
    case backingOff(until: Date)
    /// No computer that lends it could be reached.
    case unreachable
}

/// The tokens this phone borrows, one per GitHub host, and when to ask
/// for another. Sheprd stays the only holder of its refresh token: the
/// phone holds an access token until it nears its expiry or GitHub
/// refuses it, then asks the computer again, the most recently reached of
/// those that lend the host first.
public actor GitHubLeases {
    private let store: TokenStore
    private let now: @Sendable () -> Date
    /// Leases read this run, so a read does not go to the Keychain.
    private var held: [GitHubHost: GitHubLease] = [:]
    private var backoff: [GitHubHost: Backoff] = [:]
    /// How many times each computer was forgotten, so an answer it gave
    /// while being forgotten cannot put its token back.
    private var forgets: [String: Int] = [:]

    init(store: TokenStore, now: @escaping @Sendable () -> Date = { Date() }) {
        self.store = store
        self.now = now
    }

    /// How long a host's lenders are left alone after one said to wait.
    struct Backoff {
        var failures: Int
        var until: Date

        static let first: TimeInterval = 30
        static let longest: TimeInterval = 15 * 60

        /// The wait after one more refusal, doubling from `first` and, for
        /// a computer's rate limit, from twice that.
        static func next(after previous: Backoff?, status: Int, now: Date) -> Backoff {
            let failures = (previous?.failures ?? 0) + 1
            let base = status == 429 ? first * 2 : first
            let wait = min(base * pow(2, Double(failures - 1)), longest)
            return Backoff(failures: failures, until: now.addingTimeInterval(wait))
        }
    }

    /// A token for `host`: the one held while it is fresh, else a new one
    /// from `lenders`. A held token that is near its expiry but still valid
    /// is used when no computer gives a new one.
    func lease(for host: GitHubHost, lenders: [GitHubLender], fetch: FetchLease) async throws -> GitHubLease {
        reconcile(lenders)
        let current = cached(host)
        let now = now()
        if let current, current.fresh(at: now) { return current }
        let fallback = current.flatMap { $0.valid(at: now) ? $0 : nil }
        if let wait = backoff[host], wait.until > now {
            if let fallback { return fallback }
            throw LeaseError.backingOff(until: wait.until)
        }
        let candidates = GitHubLender.order(lenders, for: host)
        guard !candidates.isEmpty else {
            if let fallback { return fallback }
            throw LeaseError.noLender
        }
        var failure = LeaseError.refused
        for lender in candidates {
            try Task.checkCancellation()
            let answer: LeaseFetch
            let forgotten = forgets[lender.device, default: 0]
            do {
                answer = try await fetch(lender.device, host)
            } catch is CancellationError {
                throw CancellationError()
            } catch {
                if failure == .refused { failure = .unreachable }
                continue
            }
            guard forgets[lender.device, default: 0] == forgotten else { continue }
            switch answer {
            case let .lease(answer):
                guard GitHubHost(answer.host) == host, !answer.token.isEmpty else {
                    if failure == .refused { failure = .unreachable }
                    continue
                }
                let lease = GitHubLease(
                    host: host, login: answer.login, token: answer.token,
                    expiresAt: answer.expiresAt.map { Date(timeIntervalSince1970: TimeInterval($0)) },
                    source: answer.source, lender: lender.device, lenderName: lender.name, received: self.now()
                )
                backoff[host] = nil
                keep(lease)
                return lease
            case .status(404), .status(403):
                // That computer lends this host no more: what it lent goes.
                if held[host]?.lender == lender.device { drop(host) }
            case let .status(status) where status == 429 || status == 504:
                let next = Backoff.next(after: backoff[host], status: status, now: self.now())
                backoff[host] = next
                failure = .backingOff(until: next.until)
            case .status:
                if failure == .refused { failure = .unreachable }
            }
        }
        if let fallback = cached(host).flatMap({ $0.valid(at: self.now()) ? $0 : nil }) { return fallback }
        throw failure
    }

    /// GitHub refused `lease` (401): its computer has renewed it, or it
    /// was revoked. The next read asks for another.
    func rejected(_ lease: GitHubLease) {
        guard cached(lease.host)?.token == lease.token else { return }
        drop(lease.host)
    }

    /// Forgets leases from computers no longer paired, or no longer
    /// lending their host.
    func reconcile(_ lenders: [GitHubLender]) {
        for host in store.reconcile(lenders) { held[host] = nil }
        for (host, lease) in held where !lenders.contains(where: { lender in
            lender.device == lease.lender && (lender.hosts?.contains(host) ?? true)
        }) {
            held[host] = nil
        }
    }

    /// Forgets everything computer `device` lent, as forgetting it does.
    public func forget(lender device: String) {
        forgets[device, default: 0] += 1
        store.forget(lender: device)
        held = held.filter { $0.value.lender != device }
    }

    /// Every lease held, for Settings.
    public func all() -> [GitHubLease] {
        store.all()
    }

    private func cached(_ host: GitHubHost) -> GitHubLease? {
        if let lease = held[host] { return lease }
        let lease = store.lease(for: host)
        held[host] = lease
        return lease
    }

    private func keep(_ lease: GitHubLease) {
        held[lease.host] = lease
        // A lease kept only in memory still serves this run.
        try? store.save(lease)
    }

    private func drop(_ host: GitHubHost) {
        held[host] = nil
        try? store.forget(host)
    }
}
