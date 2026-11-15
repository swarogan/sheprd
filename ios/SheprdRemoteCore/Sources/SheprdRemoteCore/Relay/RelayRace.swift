import Foundation

/// Tries candidates in order without waiting on one that does not answer:
/// the next starts as soon as the one before fails, or after `stagger` if
/// it has done neither. The first to open wins, and the rest are cancelled.
/// So a computer's old relay that went silent costs `stagger`, not the
/// time its connection takes to give up, before a working relay is used.
enum RelayRace {
    /// How long a relay has to open before the next is tried beside it.
    static let stagger: Duration = .seconds(1)

    private enum Event<Value: Sendable>: Sendable {
        case opened(Value)
        case failed(Error)
        /// `stagger` passed since the last candidate started.
        case waited
        /// A wait cut short by the race ending.
        case stopped
    }

    /// The first of `candidates` that `open` opens. `open` must end when
    /// its task is cancelled. A candidate that opens after another won is
    /// handed to `discard`. With none opened, throws the last failure, or
    /// `none` for no candidate at all.
    static func first<Candidate: Sendable, Value: Sendable>(
        of candidates: [Candidate],
        stagger: Duration = stagger,
        none: Error,
        open: @escaping @Sendable (Candidate) async throws -> Value,
        discard: @escaping @Sendable (Value) async -> Void
    ) async throws -> Value {
        let won: Result<Value, Error> = await withTaskGroup(of: Event<Value>.self) { group in
            var waiting = candidates.makeIterator()
            var running = 0
            var last = none
            // Starts the next candidate and the wait after it; false when
            // none is left.
            func start() -> Bool {
                guard let candidate = waiting.next() else { return false }
                running += 1
                group.addTask {
                    do { return .opened(try await open(candidate)) } catch { return .failed(error) }
                }
                group.addTask {
                    (try? await Task.sleep(for: stagger)) == nil ? .stopped : .waited
                }
                return true
            }
            _ = start()
            while running > 0, let event = await group.next() {
                switch event {
                case let .opened(value):
                    group.cancelAll()
                    for await late in group {
                        if case let .opened(other) = late { await discard(other) }
                    }
                    return .success(value)
                case let .failed(error):
                    running -= 1
                    last = error
                    // Nothing is left to wait for, so the next goes now.
                    // While another still runs, its wait starts the next.
                    if running == 0 { _ = start() }
                case .waited:
                    _ = start()
                case .stopped:
                    break
                }
            }
            group.cancelAll()
            return .failure(last)
        }
        return try won.get()
    }
}
