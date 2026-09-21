import CAimuxFFI
import Foundation

// Only native data and a locked handle cross executors. User closures remain
// on the caller's thread or MainActor; this class never stores one.
final class OperationTransport: @unchecked Sendable {
    private let lock = NSLock()
    private var handle: UInt64
    init(_ handle: UInt64) { self.handle = handle }
    private func current() -> UInt64 { lock.lock(); defer { lock.unlock() }; return handle }
    var closed: Bool { current() == 0 }
    func cancel() {
        let h = current()
        if h != 0, let e = aimux_operation_cancel(h) { aimux_error_free(e) }
    }
    func close() {
        lock.lock(); let h = handle; handle = 0; lock.unlock()
        if h != 0 { aimux_operation_drop(h) }
    }
    deinit { let h = handle; if h != 0 { DispatchQueue.global().async { aimux_operation_drop(h) } } }
    func next(_ lane: Int32) throws -> String? {
        let h = current(); if h == 0 { return nil }
        var output: UnsafeMutablePointer<CChar>?; var state: Int32 = 0
        if let e = aimux_operation_next(h, lane, -1, &output, &state) {
            let error = expectAimuxError(e, context: "operation_next")
            if closed { return nil }; throw error
        }
        if state == 2 { return nil }
        guard state == 0 else { throw invariant("concurrent operation reader") }
        return takeCString(output)
    }
    func reply(_ id: String, _ reply: [String: Any]) throws {
        let wire = String(decoding: try JSONSerialization.data(withJSONObject: reply), as: UTF8.self)
        let h = current(); if h == 0 { return }
        var status: Int32 = 0
        if let e = aimux_operation_reply(h, id, wire, &status) { throw expectAimuxError(e, context: "operation_reply") }
        guard status == 0 || status == 3 else { throw invariant("unexpected operation reply status") }
    }
    func wait(_ lane: Int32) async throws -> String? {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async { continuation.resume(with: Result { try self.next(lane) }) }
        }
    }
    func closeAsync() async {
        cancel()
        await withCheckedContinuation { continuation in
            DispatchQueue.global().async { self.close(); continuation.resume() }
        }
    }
}

private struct HostEvent: Decodable {
    let type: String
    let request_id: String?
    let context: ToolCallRepairContext?
    let part: JSONValue?
    let result: JSONValue?
}
private func replyValue(_ value: RawToolCall?) throws -> [String: Any] {
    guard let value else { return ["type": "unchanged"] }
    return ["type": "repaired", "tool_call": try JSONSerialization.jsonObject(with: JSONEncoder().encode(value))]
}
private func event(_ wire: String) throws -> HostEvent { try JSONDecoder().decode(HostEvent.self, from: Data(wire.utf8)) }
private func output(_ event: HostEvent) throws -> String {
    guard let value = event.part ?? event.result else { throw invariant("missing operation output") }
    return String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
}

func runHostOperation(_ op: OperationTransport, repair: ToolCallRepair, emit: (String) throws -> Void) throws {
    defer { op.close() }
    while let wire = try op.next(2) {
        let message = try event(wire)
        if message.type == "repair_request" {
            guard let context = message.context, let id = message.request_id else { throw invariant("missing repair context") }
            let reply: [String: Any]
            do { reply = try replyValue(repair(context)) }
            catch { reply = ["type": "failed", "message": String(describing: error)] }
            try op.reply(id, reply)
        } else { try emit(output(message)) }
    }
}

@MainActor
private final class AsyncHostDriver {
    let transport: OperationTransport
    let control: Task<Void, Never>
    let terminal: Task<Void, Never>
    init(_ transport: OperationTransport, repair: @escaping AsyncToolCallRepair) {
        self.transport = transport
        let control = Task { @MainActor in
            do {
                while let wire = try await transport.wait(0) {
                    if Task.isCancelled || transport.closed { return }
                    let message = try event(wire)
                    guard let context = message.context, let id = message.request_id else { throw invariant("missing repair context") }
                    let reply: [String: Any]
                    do { reply = try replyValue(await repair(context)) }
                    catch { reply = ["type": "failed", "message": String(describing: error)] }
                    if Task.isCancelled || transport.closed { return }
                    try transport.reply(id, reply)
                }
            } catch { transport.cancel() }
        }
        self.control = control
        terminal = Task { _ = try? await transport.wait(3); control.cancel() }
    }
    deinit { control.cancel(); terminal.cancel(); transport.cancel() }
    func next() async throws -> String? {
        try await withTaskCancellationHandler {
            guard let wire = try await transport.wait(1) else { return nil }
            let message = try event(wire)
            return try output(message)
        } onCancel: { [transport] in transport.cancel() }
    }
    func close() async {
        control.cancel(); await transport.closeAsync(); await terminal.value
        // Cancellation is cooperative: a user closure may ignore it. Its late
        // result is discarded above and cannot access the closed native owner.
    }
}

public extension Model {
    @MainActor
    private func hostResult<T: Decodable>(_ type: T.Type, mode: String, prompt: ModelPrompt, options: GenerateTextOptions?, repair: @escaping AsyncToolCallRepair) async throws -> T {
        let driver = AsyncHostDriver(try startHostOperation(mode: mode, prompt: prompt, options: options), repair: repair)
        do {
            guard let wire = try await driver.next() else { throw invariant("operation ended without result") }
            let result = try JSONDecoder().decode(T.self, from: Data(wire.utf8))
            await driver.close(); return result
        } catch { await driver.close(); throw error }
    }
    @MainActor
    private func hostStream<T: Decodable>(_ type: T.Type, mode: String, prompt: ModelPrompt, options: GenerateTextOptions?, repair: @escaping AsyncToolCallRepair) -> AsyncThrowingStream<T, Error> {
        do {
            let driver = AsyncHostDriver(try startHostOperation(mode: mode, prompt: prompt, options: options), repair: repair)
            return AsyncThrowingStream(unfolding: {
                do {
                    guard let wire = try await driver.next() else { await driver.close(); return nil }
                    return try JSONDecoder().decode(T.self, from: Data(wire.utf8))
                } catch { await driver.close(); throw error }
            })
        } catch { return AsyncThrowingStream { $0.finish(throwing: error) } }
    }
    @MainActor
    func generateText(prompt: ModelPrompt, options: GenerateTextOptions? = nil, repairToolCall: @escaping AsyncToolCallRepair) async throws -> GenerateTextResult {
        try await hostResult(GenerateTextResult.self, mode: "generate_text", prompt: prompt, options: options, repair: repairToolCall)
    }
    @MainActor
    func generateObject(prompt: ModelPrompt, options: GenerateTextOptions? = nil, repairToolCall: @escaping AsyncToolCallRepair) async throws -> GenerateObjectResult {
        try await hostResult(GenerateObjectResult.self, mode: "generate_object", prompt: prompt, options: options, repair: repairToolCall)
    }
    @MainActor
    func consumeStreamText(prompt: ModelPrompt, options: GenerateTextOptions? = nil, repairToolCall: @escaping AsyncToolCallRepair) async throws -> StreamTextResultAggregated {
        try await hostResult(StreamTextResultAggregated.self, mode: "consume_stream_text", prompt: prompt, options: options, repair: repairToolCall)
    }
    @MainActor
    func generateTextAsOpenAI(prompt: ModelPrompt, options: GenerateTextOptions? = nil, repairToolCall: @escaping AsyncToolCallRepair) async throws -> ChatCompletion {
        try await hostResult(ChatCompletion.self, mode: "generate_text_as_openai", prompt: prompt, options: options, repair: repairToolCall)
    }
    @MainActor
    func streamTextAsync(prompt: ModelPrompt, options: GenerateTextOptions? = nil, repairToolCall: @escaping AsyncToolCallRepair) -> AsyncThrowingStream<StreamPart, Error> {
        hostStream(StreamPart.self, mode: "stream_text", prompt: prompt, options: options, repair: repairToolCall)
    }
    @MainActor
    func streamTextAsOpenAIAsync(prompt: ModelPrompt, options: GenerateTextOptions? = nil, repairToolCall: @escaping AsyncToolCallRepair) -> AsyncThrowingStream<ChatCompletionChunk, Error> {
        hostStream(ChatCompletionChunk.self, mode: "stream_text_as_openai", prompt: prompt, options: options, repair: repairToolCall)
    }
}
