import Foundation
import SheprdRemoteCore
import os

private let log = Logger(subsystem: "so.pen.herdr.remote", category: "relay")

/// Gets this phone's relay keys to a computer it cannot reach, through one
/// it can (`HostRelay`). A computer that turned on relays while the phone
/// was away otherwise stays out of reach until the phone meets it on its
/// network again. The phone signs fresh keys with its device key for the
/// far computer and posts them to an online one, whose Sheprd carries them
/// over SSH; the far computer answers with its relays, signed with the
/// certificate this phone pinned for it, which become its relay record
/// here.
@MainActor
final class RelayCarrier {
    typealias Carry = (_ near: String, _ request: API.HostRelayRequest) async throws -> API.HostRelayState

    /// How often a pending request is read again.
    private static let poll: Duration = .seconds(3)
    /// How long the phone follows one request: Sheprd may take a few
    /// minutes to pick it up, then tries hosts over SSH one after another
    /// for up to eight (`host_relay::CARRY_MS`).
    private static let patience: Duration = .seconds(15 * 60)

    private var plan = HostRelayPlan()
    /// Bumped for a computer reached directly or forgotten, so an attempt
    /// that started before then saves nothing: the phone may have sent
    /// newer keys to it directly since.
    private var generations: [String: Int] = [:]
    private let carry: Carry
    private let onRelay: (String, RelayRecord) -> Void

    init(carry: @escaping Carry, onRelay: @escaping (String, RelayRecord) -> Void) {
        self.carry = carry
        self.onRelay = onRelay
    }

    /// Starts an attempt for each computer that is out of reach, has no
    /// relay record, and has an online computer to ask.
    func check(_ machines: [(computer: Computer, status: LinkStatus)]) {
        for (computer, status) in machines {
            if case .online = status { forget(computer.device) }
        }
        for pick in plan.due(machines, at: .now) {
            Task {
                let settled = await follow(pick)
                plan.finished(pick.far.device, settled: settled, at: .now)
            }
        }
    }

    func forget(_ device: String) {
        plan.forget(device)
        generations[device, default: 0] += 1
    }

    /// Whether an attempt started at `generation` still speaks for `device`.
    private func current(_ device: String, _ generation: Int) -> Bool {
        generations[device, default: 0] == generation
    }

    /// Follows one attempt; whether its answer holds for a while.
    private func follow(_ pick: HostRelayPlan.Pick) async -> Bool {
        let far = pick.far
        let generation = generations[far.device, default: 0]
        let keys = RelayKeyPair()
        let request: API.HostRelayRequest
        do {
            request = try Self.sign(keys, for: far)
        } catch {
            log.error("could not sign relay keys for \(far.name, privacy: .public): \(error.localizedDescription, privacy: .public)")
            return false
        }
        let deadline = ContinuousClock.now + Self.patience
        while ContinuousClock.now < deadline, current(far.device, generation) {
            let state: API.HostRelayState
            do {
                state = try await carry(pick.near.device, request)
            } catch RemoteError.status(429, _) {
                // Older keys for it are still being carried.
                state = .pending
            } catch {
                log.info("\(pick.near.name, privacy: .public) did not carry relay keys to \(far.name, privacy: .public): \(error.localizedDescription, privacy: .public)")
                return false
            }
            switch state {
            case .pending:
                do { try await Task.sleep(for: Self.poll) } catch { return false }
            case let .relayed(info, proof):
                guard HostRelay.proven(proof, info: info, for: request) else {
                    log.error("an answer for \(far.name, privacy: .public) was not signed by it")
                    return false
                }
                // Reached directly meanwhile: what it holds now came from
                // there, and these keys would replace them here.
                guard current(far.device, generation) else { return false }
                guard let record = RelayRecord.make(from: info, keys: keys) else { return false }
                log.notice("\(pick.near.name, privacy: .public) carried relay keys to \(far.name, privacy: .public)")
                onRelay(far.device, record)
                return true
            case let .kept(proof):
                guard HostRelay.proven(proof, info: nil, for: request) else {
                    log.error("an answer for \(far.name, privacy: .public) was not signed by it")
                    return false
                }
                log.info("\(far.name, privacy: .public) keeps relay keys but listens on no relay")
                return true
            case .elsewhere:
                log.info("\(pick.near.name, privacy: .public) does not connect to \(far.name, privacy: .public)")
                return true
            case let .failed(reason):
                log.error("\(pick.near.name, privacy: .public) could not carry relay keys to \(far.name, privacy: .public): \(reason, privacy: .public)")
                return false
            }
        }
        return false
    }

    /// `keys`, signed with this phone's key for `far`.
    private static func sign(_ keys: RelayKeyPair, for far: Computer) throws -> API.HostRelayRequest {
        guard let enrollment = keys.enrollment else { throw Failure.keys }
        let signedMs = UInt64(Date.now.timeIntervalSince1970 * 1000)
        guard let message = HostRelay.message(
            fingerprint: far.fingerprint, device: far.device, keys: enrollment, signedMs: signedMs
        ) else { throw Failure.keys }
        let key = try DeviceKey(kind: far.keyKind, data: far.keyData)
        return API.HostRelayRequest(
            fingerprint: far.fingerprint.description,
            device: far.device,
            keys: enrollment,
            signedMs: signedMs,
            signature: DeviceKeyEncoding.signature(try key.signature(for: message))
        )
    }

    enum Failure: Error, LocalizedError {
        case keys

        var errorDescription: String? { "Could not make relay keys." }
    }
}
