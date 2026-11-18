import CoreBluetooth
import Foundation

/// The other half of nearby pairing, for adding another computer's Sheprd
/// that is on another network (docs/relay-plan.md, Adding a nearby Sheprd
/// from another network): this Mac plays the phone. It scans for
/// computers whose Pair device dialog is open, reads each one's `info`,
/// and carries `pair` frames to the one the user picks, framed exactly as
/// a phone frames them. As on the peripheral side, it holds no secrets and
/// decides nothing: the code exchange runs in `sheprd-remote-control
/// desktop-pair`, and Sheprd carries its frames.
@MainActor
final class Central: NSObject {
    private var manager: CBCentralManager?
    private var scanning = false
    /// Every computer connected to read its `info`, or to pair, by id.
    private var peripherals: [UUID: CBPeripheral] = [:]
    /// The `pair` characteristic of each computer that is ready to carry
    /// frames.
    private var pairs: [UUID: CBCharacteristic] = [:]
    /// The computers to subscribe to once their characteristics are known.
    private var wanted: Set<UUID> = []
    private var incoming: [UUID: Reassembly] = [:]
    /// Chunks waiting for the write before them to be acknowledged.
    private var pending: [UUID: [Data]] = [:]
    /// Each computer's `info`, read again while scanning: a computer seen
    /// before its Pair device opened names itself only then, and one whose
    /// dialog closed stops naming itself.
    private var infos: [UUID: CBCharacteristic] = [:]
    /// The computers Sheprd was told about with `found`, which it is told
    /// about again when they leave.
    private var listed: Set<UUID> = []
    private var refresh: Timer?

    private static let service = CBUUID(string: IDs.service)
    private static let info = CBUUID(string: IDs.info)
    private static let pair = CBUUID(string: IDs.pair)

    /// Starts or stops looking for computers that are pairing. Stopping
    /// leaves alone the one being paired with.
    func scan(_ on: Bool) {
        scanning = on
        if let manager {
            // A page opened again learns the radio at once, not at its next
            // change; a new manager reports it on its own.
            if on { reportState(manager.state) }
        } else {
            manager = CBCentralManager(delegate: self, queue: nil)
        }
        apply()
        refresh?.invalidate()
        refresh = nil
        if on {
            refresh = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { _ in
                MainActor.assumeIsolated { self.readInfos() }
            }
        } else {
            for (id, peripheral) in peripherals where !wanted.contains(id) {
                manager?.cancelPeripheralConnection(peripheral)
                left(id)
                forget(id)
            }
        }
    }

    /// Reads `info` again from every computer found and not being paired.
    private func readInfos() {
        for (id, info) in infos where !wanted.contains(id) {
            guard let peripheral = peripherals[id], peripheral.state == .connected else { continue }
            peripheral.readValue(for: info)
        }
    }

    /// Tells Sheprd that `id`, if it was listed or being paired, is gone.
    private func left(_ id: UUID) {
        let known = listed.remove(id) != nil
        if wanted.remove(id) != nil || known {
            emit(["type": "peripheral_disconnected", "peripheral": id.uuidString])
        }
    }

    private func reportState(_ state: CBManagerState) {
        let names: [CBManagerState: String] = [
            .poweredOn: "powered_on", .poweredOff: "powered_off", .unauthorized: "unauthorized",
            .unsupported: "unsupported", .resetting: "resetting",
        ]
        emit(["type": "central_state", "state": names[state] ?? "unknown"])
    }

    /// Subscribes to `id`'s `pair` characteristic; `connected` follows.
    func connect(_ id: UUID) {
        guard let peripheral = peripherals[id] else {
            emit(["type": "peripheral_disconnected", "peripheral": id.uuidString])
            return
        }
        wanted.insert(id)
        if let pair = pairs[id] {
            peripheral.setNotifyValue(true, for: pair)
        } else if peripheral.state == .connected {
            peripheral.discoverServices([Self.service])
        } else {
            manager?.connect(peripheral)
        }
    }

    /// Lets go of the computer being paired with. It stays listed while
    /// scanning, as another attempt may pick it again.
    func disconnect(_ id: UUID) {
        wanted.remove(id)
        guard let peripheral = peripherals[id] else { return }
        if scanning {
            if let pair = pairs[id] { peripheral.setNotifyValue(false, for: pair) }
            incoming[id] = nil
            pending[id] = nil
        } else {
            manager?.cancelPeripheralConnection(peripheral)
            listed.remove(id)
            forget(id)
        }
    }

    /// Writes one message to `id`, in chunks it acknowledges one by one.
    func send(_ message: Data, to id: UUID) {
        guard let peripheral = peripherals[id], pairs[id] != nil else { return }
        let size = peripheral.maximumWriteValueLength(for: .withResponse)
        let queued = pending[id, default: []]
        pending[id] = queued + Framing.chunks(message, size: size)
        if queued.isEmpty { writeNext(id) }
    }

    private func writeNext(_ id: UUID) {
        guard let peripheral = peripherals[id], let pair = pairs[id], let chunk = pending[id]?.first else { return }
        peripheral.writeValue(chunk, for: pair, type: .withResponse)
    }

    private func apply() {
        guard let manager, manager.state == .poweredOn else { return }
        if scanning, !manager.isScanning {
            manager.scanForPeripherals(withServices: [Self.service])
        } else if !scanning, manager.isScanning {
            manager.stopScan()
        }
    }

    private func forget(_ id: UUID) {
        peripherals[id] = nil
        pairs[id] = nil
        infos[id] = nil
        incoming[id] = nil
        pending[id] = nil
    }
}

extension Central {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        reportState(central.state)
        apply()
    }

    func centralManager(
        _ central: CBCentralManager, didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any], rssi RSSI: NSNumber
    ) {
        guard peripherals[peripheral.identifier] == nil else { return }
        peripherals[peripheral.identifier] = peripheral
        peripheral.delegate = self
        central.connect(peripheral)
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        peripheral.discoverServices([Self.service])
    }

    func centralManager(
        _ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?
    ) {
        left(peripheral.identifier)
        forget(peripheral.identifier)
    }

    func centralManager(
        _ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?
    ) {
        left(peripheral.identifier)
        forget(peripheral.identifier)
    }
}

extension Central {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard let service = peripheral.services?.first(where: { $0.uuid == Self.service }) else { return }
        peripheral.discoverCharacteristics([Self.info, Self.pair], for: service)
    }

    func peripheral(
        _ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?
    ) {
        let id = peripheral.identifier
        for characteristic in service.characteristics ?? [] {
            if characteristic.uuid == Self.info {
                infos[id] = characteristic
                peripheral.readValue(for: characteristic)
            } else if characteristic.uuid == Self.pair {
                pairs[id] = characteristic
                if wanted.contains(id) { peripheral.setNotifyValue(true, for: characteristic) }
            }
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic, error: Error?
    ) {
        let id = peripheral.identifier.uuidString
        guard characteristic.uuid == Self.pair, wanted.contains(peripheral.identifier) else { return }
        if error == nil, characteristic.isNotifying {
            emit(["type": "connected", "peripheral": id])
        } else {
            emit(["type": "peripheral_disconnected", "peripheral": id])
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?
    ) {
        let id = peripheral.identifier
        if characteristic.uuid == Self.info {
            // Only a computer showing Pair device names itself; one
            // advertising for presence alone does not, nor does one whose
            // dialog closed, which refuses the read.
            guard
                error == nil, let value = characteristic.value,
                let fields = try? JSONSerialization.jsonObject(with: value) as? [String: Any],
                let name = fields["name"] as? String
            else {
                if !wanted.contains(id), listed.remove(id) != nil {
                    emit(["type": "peripheral_disconnected", "peripheral": id.uuidString])
                }
                return
            }
            guard listed.insert(id).inserted else { return }
            emit([
                "type": "found", "peripheral": id.uuidString, "name": String(name.prefix(64)),
                "fp": (fields["fp"] as? String) ?? "",
            ])
        } else if characteristic.uuid == Self.pair {
            guard error == nil, let value = characteristic.value else { return }
            var assembly = incoming[id] ?? Reassembly()
            switch assembly.add(value) {
            case let .complete(message):
                incoming[id] = nil
                emit(["type": "peripheral_frame", "peripheral": id.uuidString, "data": message.base64EncodedString()])
            case .partial:
                incoming[id] = assembly
            case .invalid:
                incoming[id] = nil
            }
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral, didWriteValueFor characteristic: CBCharacteristic, error: Error?
    ) {
        let id = peripheral.identifier
        if error != nil {
            pending[id] = nil
            emit(["type": "error", "message": "write: \(error?.localizedDescription ?? "")"])
            return
        }
        pending[id]?.removeFirst()
        writeNext(id)
    }
}

// Central callbacks arrive on the main queue (`queue: nil`), as the
// peripheral's do in main.swift.
#if compiler(>=6.0)
extension Central: @preconcurrency CBCentralManagerDelegate, @preconcurrency CBPeripheralDelegate {}
#else
extension Central: CBCentralManagerDelegate, CBPeripheralDelegate {}
#endif
