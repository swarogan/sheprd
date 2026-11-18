import Foundation
import Security
import Synchronization

/// Secret items by account name: the Keychain on a phone, memory in tests.
struct SecretVault: Sendable {
    var read: @Sendable (_ account: String) throws -> Data?
    var write: @Sendable (_ data: Data, _ account: String) throws -> Void
    var delete: @Sendable (_ account: String) throws -> Void
    var accounts: @Sendable () throws -> [String]

    init(
        read: @escaping @Sendable (String) throws -> Data?,
        write: @escaping @Sendable (Data, String) throws -> Void,
        delete: @escaping @Sendable (String) throws -> Void,
        accounts: @escaping @Sendable () throws -> [String]
    ) {
        self.read = read
        self.write = write
        self.delete = delete
        self.accounts = accounts
    }

    /// Items kept in memory, gone with the process.
    static func memory() -> SecretVault {
        final class Items: Sendable {
            let items = Mutex<[String: Data]>([:])
        }
        let held = Items()
        return SecretVault(
            read: { account in held.items.withLock { $0[account] } },
            write: { data, account in held.items.withLock { $0[account] = data } },
            delete: { account in held.items.withLock { $0[account] = nil } },
            accounts: { held.items.withLock { Array($0.keys) } }
        )
    }
}

/// The Keychain's generic passwords under one service. Items are this
/// device's only and readable after the first unlock, so reads can run
/// while the phone is locked; never synchronized to iCloud or restored to
/// another device. No access group is named, so they stay in the app's
/// own (its application identifier): the widgets, the extensions, and the
/// watch, which share the App Group, cannot read them.
enum Keychain {
    struct Failure: Error, Equatable {
        let status: OSStatus
    }

    static func vault(service: String) -> SecretVault {
        SecretVault(
            read: { account in try read(service: service, account: account) },
            write: { data, account in try write(data, service: service, account: account) },
            delete: { account in try delete(service: service, account: account) },
            accounts: { try accounts(service: service) }
        )
    }

    private static func query(service: String, account: String?) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrSynchronizable as String: false,
            // The iOS keychain on a Mac too, so `swift test` sees what a
            // phone does rather than a login keychain.
            kSecUseDataProtectionKeychain as String: true,
        ]
        if let account { query[kSecAttrAccount as String] = account }
        return query
    }

    static func read(service: String, account: String) throws -> Data? {
        var query = query(service: service, account: account)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess: return result as? Data
        case errSecItemNotFound: return nil
        default: throw Failure(status: status)
        }
    }

    static func write(_ data: Data, service: String, account: String) throws {
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        let found = query(service: service, account: account)
        let status = SecItemUpdate(found as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            let added = SecItemAdd(found.merging(attributes) { $1 } as CFDictionary, nil)
            guard added == errSecSuccess else { throw Failure(status: added) }
            return
        }
        guard status == errSecSuccess else { throw Failure(status: status) }
    }

    static func delete(service: String, account: String) throws {
        let status = SecItemDelete(query(service: service, account: account) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw Failure(status: status) }
    }

    static func accounts(service: String) throws -> [String] {
        var query = query(service: service, account: nil)
        query[kSecReturnAttributes as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitAll
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess:
            return ((result as? [[String: Any]]) ?? []).compactMap { $0[kSecAttrAccount as String] as? String }
        case errSecItemNotFound: return []
        default: throw Failure(status: status)
        }
    }
}

/// The GitHub tokens this phone holds, one item per GitHub host, each with
/// the computer that lent it, its login, source, and expiry.
struct TokenStore: Sendable {
    /// The Keychain service the app keeps them under.
    static let service = "so.pen.herdr.remote.github"

    let vault: SecretVault

    init(vault: SecretVault) {
        self.vault = vault
    }

    /// The lease held for `host`; an item this version cannot read is
    /// dropped, as a new lease replaces it anyway.
    func lease(for host: GitHubHost) -> GitHubLease? {
        guard let data = try? vault.read(host.name) else { return nil }
        guard let lease = try? JSONDecoder().decode(GitHubLease.self, from: data), lease.host == host else {
            try? vault.delete(host.name)
            return nil
        }
        return lease
    }

    func save(_ lease: GitHubLease) throws {
        try vault.write(JSONEncoder().encode(lease), lease.host.name)
    }

    func forget(_ host: GitHubHost) throws {
        try vault.delete(host.name)
    }

    /// Every lease held, for Settings.
    func all() -> [GitHubLease] {
        ((try? vault.accounts()) ?? []).sorted().compactMap(GitHubHost.init).compactMap(lease(for:))
    }

    /// Forgets each lease whose computer is no longer paired, or whose
    /// latest `home` no longer lends its host; answers the hosts forgotten.
    /// A computer whose `home` has not arrived yet keeps its leases.
    @discardableResult
    func reconcile(_ lenders: [GitHubLender]) -> [GitHubHost] {
        let byDevice = Dictionary(lenders.map { ($0.device, $0) }) { first, _ in first }
        var forgotten: [GitHubHost] = []
        for lease in all() {
            let lender = byDevice[lease.lender]
            let keeps = lender.map { $0.hosts?.contains(lease.host) ?? true } ?? false
            guard !keeps else { continue }
            try? forget(lease.host)
            forgotten.append(lease.host)
        }
        return forgotten
    }

    /// Forgets what computer `device` lent, as forgetting it does.
    func forget(lender device: String) {
        for lease in all() where lease.lender == device {
            try? forget(lease.host)
        }
    }
}
