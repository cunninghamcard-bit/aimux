import Foundation

@main struct SwiftHostSmoke {
    static func fixed(_ c: ToolCallRepairContext) -> RawToolCall? {
        var call = c.toolCall; call.input = "{\"city\":\"北京\"}"; return call
    }
    static func syncCheck(_ model: Model, _ options: GenerateTextOptions) throws {
        let caller = Thread.current
        let result = try model.generateText(prompt: .text("hello"), options: options, repairToolCall: { c in
            precondition(Thread.current === caller)
            let nested = try model.generateText(prompt: .text("nested"), options: options, repairToolCall: fixed)
            precondition(nested.toolCalls.first?.input == .object(["city": .string("北京")]))
            return fixed(c)
        })
        precondition(result.toolCalls.first?.input == .object(["city": .string("北京")]))
        print("PASS: sync caller thread and nested generation")
    }
    @MainActor static func main() async throws {
        let fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))) as! [String: Any]
        let options = try JSONDecoder().decode(GenerateTextOptions.self, from: JSONSerialization.data(withJSONObject: fixture["options"]!))
        let server = MockHTTPServer(response: .json(fixture["response"]!)); try server.start(); defer { server.stop() }
        let model = try Model.openai(apiKey: "fake", modelId: "mock", baseUrl: server.baseURL)
        try syncCheck(model, options)
        let repaired = try await model.generateText(prompt: .text("hello"), options: options, repairToolCall: { c in
            MainActor.preconditionIsolated()
            await Task.yield(); return fixed(c)
        })
        precondition(repaired.toolCalls.first?.input == .object(["city": .string("北京")]))
        print("PASS: async repair executes on MainActor")
        var timed = options; timed.timeout = .init(totalMs: 200)
        var cancelled = false
        do {
            _ = try await model.generateText(prompt: .text("hello"), options: timed, repairToolCall: { _ in
                do { try await Task.sleep(nanoseconds: 10_000_000_000) }
                catch { cancelled = true; throw error }
                return nil
            })
            fatalError("expected timeout")
        } catch let error as AimuxError { precondition(error.code == 12) }
        await Task.yield()
        precondition(cancelled)
        print("PASS: timeout cancels suspended host repair")
    }
}
