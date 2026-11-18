import Foundation

extension Tunnel {
    /// Records after the handshake, as `sheprd-tunnel`'s `record` module
    /// writes them: `flags ‖ length (u16, big-endian) ‖ data ‖ zeros`,
    /// padded to 256-byte buckets, encrypted with Noise's transport keys.
    /// A sender sets `rekey` on the record after which it rekeys (every
    /// 2^20 records or ten minutes); `close` ends the tunnel.
    public struct Transport {
        public static let pad = 256
        public static let maxData = 16 * 1024
        public static let rekeyRecords: UInt64 = 1 << 20
        public static let rekeyAfter: TimeInterval = 10 * 60
        static let rekeyFlag: UInt8 = 0b01
        static let closeFlag: UInt8 = 0b10
        static let header = 3

        public enum Opened: Equatable {
            case data(Data)
            case closed
        }

        var send: Noise.CipherState
        var receive: Noise.CipherState
        var rekeyed: Date?
        var sent: UInt64 = 0

        init(send: Noise.CipherState, receive: Noise.CipherState) {
            self.send = send
            self.receive = receive
        }

        /// Seals `data` (at most `maxData` bytes) as one record, rekeying
        /// after it when due at `now`.
        public mutating func seal(_ data: Data, now: Date = Date()) throws -> Data {
            guard data.count <= Self.maxData else { throw Failure.frameTooLong }
            let since = rekeyed ?? now
            rekeyed = since
            let due = sent + 1 >= Self.rekeyRecords || now.timeIntervalSince(since) >= Self.rekeyAfter
            let record = try seal(flags: due ? Self.rekeyFlag : 0, data)
            if due {
                try send.rekey()
                sent = 0
                rekeyed = now
            } else {
                sent += 1
            }
            return record
        }

        /// Seals the record that closes the tunnel.
        public mutating func sealClose() throws -> Data {
            try seal(flags: Self.closeFlag, Data())
        }

        mutating func seal(flags: UInt8, _ data: Data) throws -> Data {
            let length = UInt16(data.count)
            let padded = ((Self.header + data.count + Self.pad - 1) / Self.pad) * Self.pad
            var plain = Data([flags, UInt8(length >> 8), UInt8(length & 0xff)]) + data
            plain.append(Data(count: padded - plain.count))
            return try send.encrypt(plain, ad: Data())
        }

        /// Opens one record. Anything changed, replayed, reordered, or
        /// padded with other than zeros is refused.
        public mutating func open(_ record: Data) throws -> Opened {
            guard record.count >= Noise.tagLength + Self.pad, record.count <= 65_535 else { throw Failure.malformed }
            let plain: Data
            do {
                plain = try receive.decrypt(record, ad: Data())
            } catch {
                throw Failure.record
            }
            let bytes = [UInt8](plain)
            guard bytes.count >= Self.header, bytes.count % Self.pad == 0 else { throw Failure.malformed }
            let flags = bytes[0]
            let length = Int(bytes[1]) << 8 | Int(bytes[2])
            let rest = bytes[Self.header...]
            guard flags & ~(Self.rekeyFlag | Self.closeFlag) == 0,
                  length <= rest.count, length <= Self.maxData,
                  rest.dropFirst(length).allSatisfy({ $0 == 0 })
            else { throw Failure.malformed }
            if flags & Self.closeFlag != 0 { return .closed }
            let data = Data(rest.prefix(length))
            if flags & Self.rekeyFlag != 0 { try receive.rekey() }
            return .data(data)
        }
    }

    /// Framing on the relay's byte stream: a big-endian `u16` length, then
    /// that many bytes.
    public enum Frame {
        public static func encode(_ message: Data) throws -> Data {
            guard message.count <= 65_535 else { throw Failure.frameTooLong }
            return Data([UInt8(message.count >> 8), UInt8(message.count & 0xff)]) + message
        }

        /// Collects bytes as they arrive and gives back whole frames.
        public struct Decoder {
            var buffer = Data()

            public init() {}

            public mutating func push(_ bytes: Data) {
                buffer.append(bytes)
            }

            public mutating func next() -> Data? {
                guard buffer.count >= 2 else { return nil }
                let start = buffer.startIndex
                let length = Int(buffer[start]) << 8 | Int(buffer[start + 1])
                guard buffer.count >= 2 + length else { return nil }
                let frame = Data(buffer[(start + 2)..<(start + 2 + length)])
                buffer = Data(buffer.dropFirst(2 + length))
                return frame
            }
        }
    }
}
