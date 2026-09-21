// ToolCallRepairTests.swift — host-side tool-call repair (RFC-0035).
//
// Two layers:
//   • the shared fixture `contract-tests/fixtures/tool-call-repair.json`
//     replayed through the three pure native functions, the same file Rust and
//     the other bindings assert against;
//   • the typed `GenerateTextOptions.repairToolCall` loop end-to-end against
//     `MockHTTPServer`, which is where the branches a host actually sees live
//     (repaired / unchanged / failed / still invalid / never invoked).

import Foundation
import XCTest
@testable import Aimux

final class ToolCallRepairTests: XCTestCase {

    // MARK: - shared fixture

    /// <repo>/contract-tests/fixtures/tool-call-repair.json
    private static var fixtureURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // AimuxTests
            .deletingLastPathComponent()   // Tests
            .deletingLastPathComponent()   // swift
            .deletingLastPathComponent()   // bindings
            .deletingLastPathComponent()   // <repo>
            .appendingPathComponent("contract-tests/fixtures/tool-call-repair.json")
    }

    /// Every fixture case runs through the matching native function; a case
    /// whose `function` has no arm here fails rather than being skipped.
    func testSharedFixtureCases() throws {
        let data = try Data(contentsOf: Self.fixtureURL)
        let document = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let cases = document?["cases"] as? [[String: Any]] ?? []
        XCTAssertFalse(cases.isEmpty, "no repair fixture cases loaded")

        for testCase in cases {
            let name = testCase["name"] as? String ?? "<unnamed>"
            let input = testCase["input"] as? [String: Any] ?? [:]
            let opts = json(input["opts"] ?? NSNull())
            let optsJson = opts == "null" ? nil : opts

            let run: () throws -> String
            switch testCase["function"] as? String {
            case "tool_call_repair_context":
                run = { try toolCallRepairContext(
                    toolCall: json(input["tool_call"]!), prompt: json(input["prompt"]!),
                    options: optsJson
                ) }
            case "apply_tool_call_repair":
                run = { try applyToolCallRepair(
                    toolCall: json(input["tool_call"]!), options: optsJson,
                    reply: json(input["reply"]!)
                ) }
            case "apply_tool_call_repair_to_result":
                run = { try applyToolCallRepairToResult(
                    result: json(input["result"]!), options: optsJson,
                    toolCallId: input["tool_call_id"] as? String ?? "",
                    reply: json(input["reply"]!)
                ) }
            default:
                XCTFail("fixture case '\(name)' declares an unknown function — wire it up")
                continue
            }

            if let expectedError = testCase["expected_error"] as? String {
                XCTAssertThrowsError(try run(), "case '\(name)' should fail") { error in
                    XCTAssertEqual(expectedError, "InvalidArgument", "case '\(name)'")
                    guard case .invalidArgument = error as? AimuxError else {
                        return XCTFail("case '\(name)': expected .invalidArgument, got \(error)")
                    }
                }
                continue
            }
            let actual = try run()
            assertJSONEqual(actual, testCase["expected"] ?? NSNull(), case: name)
        }
    }

    // MARK: - the typed loop, end to end

    /// A corrected call comes back valid, and the replayed transcript is
    /// patched with it — otherwise the next turn sends the model the arguments
    /// it already got wrong.
    func testRepairedCallPatchesResultAndTranscript() throws {
        let server = MockHTTPServer(response: .json(openaiInvalidToolCallResponse))
        try server.start()
        defer { server.stop() }

        var seen: ToolCallRepairContext?
        let result = try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .generateText(prompt: .text("weather in Singapore?"),
                          options: weatherOptions { context in
                              seen = context
                              return RawToolCall(toolCallId: context.toolCall.toolCallId,
                                                 toolName: context.toolCall.toolName,
                                                 input: #"{"city":"Singapore"}"#)
                          })

        // The repair function saw the provider's raw argument text and the
        // failure the call carried.
        XCTAssertEqual(seen?.toolCall.input, #"{"town":"Singapore"}"#)
        XCTAssertNotNil(seen?.error["InvalidToolInput"])
        XCTAssertEqual(seen?.inputSchema["required"]?[0]?.stringValue, "city")
        XCTAssertEqual(seen?.tools.count, 1)
        XCTAssertEqual(seen?.messages.count, 1)

        XCTAssertEqual(result.toolCalls.count, 1)
        XCTAssertNotEqual(result.toolCalls[0].invalid, true)
        XCTAssertNil(result.toolCalls[0].error)
        XCTAssertEqual(result.toolCalls[0].input["city"]?.stringValue, "Singapore")

        XCTAssertEqual(transcriptToolCallInput(result.responseMessages)?["city"]?.stringValue,
                       "Singapore")
    }

    /// Returning `nil` is the AI SDK's `Ok(None)`: the call keeps its original
    /// error untouched.
    func testUnchangedKeepsTheInvalidCall() throws {
        let server = MockHTTPServer(response: .json(openaiInvalidToolCallResponse))
        try server.start()
        defer { server.stop() }

        let result = try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .generateText(prompt: .text("weather in Singapore?"),
                          options: weatherOptions { _ in nil })

        XCTAssertEqual(result.toolCalls[0].invalid, true)
        XCTAssertNotNil(result.toolCalls[0].error?["InvalidToolInput"])
    }

    /// Throwing is the AI SDK's `Err(e)`: the call stays invalid carrying a
    /// `ToolCallRepair` error whose cause is the thrown message.
    func testThrowingRepairYieldsToolCallRepairError() throws {
        struct RepairUnavailable: LocalizedError {
            var errorDescription: String? { "repair model unavailable" }
        }
        let server = MockHTTPServer(response: .json(openaiInvalidToolCallResponse))
        try server.start()
        defer { server.stop() }

        let result = try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .generateText(prompt: .text("weather in Singapore?"),
                          options: weatherOptions { _ in throw RepairUnavailable() })

        XCTAssertEqual(result.toolCalls[0].invalid, true)
        guard let repairError = result.toolCalls[0].error?["ToolCallRepair"] else {
            return XCTFail("expected a ToolCallRepair error, got \(String(describing: result.toolCalls[0].error))")
        }
        XCTAssertTrue(String(describing: repairError).contains("repair model unavailable"),
                      "the thrown message must survive as the repair cause")
    }

    /// A replacement that still fails validation is re-reported, not trusted.
    func testRepairedButStillInvalid() throws {
        let server = MockHTTPServer(response: .json(openaiInvalidToolCallResponse))
        try server.start()
        defer { server.stop() }

        let result = try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .generateText(prompt: .text("weather in Singapore?"),
                          options: weatherOptions { context in
                              RawToolCall(toolCallId: context.toolCall.toolCallId,
                                          toolName: context.toolCall.toolName,
                                          input: #"{"village":"Singapore"}"#)
                          })

        XCTAssertEqual(result.toolCalls[0].invalid, true)
        XCTAssertNotNil(result.toolCalls[0].error?["ToolCallRepair"])
    }

    /// A call made without a tool set is never repairable (AI SDK rule), so the
    /// function is not invoked — the library says so, the host does not guess.
    func testCallWithoutToolsNeverReachesTheRepairFunction() throws {
        let server = MockHTTPServer(response: .json(openaiInvalidToolCallResponse))
        try server.start()
        defer { server.stop() }

        var invoked = false
        var options = GenerateTextOptions()
        options.repairToolCall = { _ in invoked = true; return nil }
        _ = try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .generateText(prompt: .text("weather in Singapore?"), options: options)

        XCTAssertFalse(invoked, "a call generated without tools is never repaired")
    }

    /// A call that parsed and validated is not invalid, so nothing is repaired.
    func testValidCallNeverReachesTheRepairFunction() throws {
        let server = MockHTTPServer(response: .json(openaiValidToolCallResponse))
        try server.start()
        defer { server.stop() }

        var invoked = false
        let result = try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .generateText(prompt: .text("weather in Singapore?"),
                          options: weatherOptions { _ in invoked = true; return nil })

        XCTAssertFalse(invoked)
        XCTAssertEqual(result.toolCalls[0].input["city"]?.stringValue, "Singapore")
    }

    /// The repair function runs outside every native call, so it may issue a
    /// second aimux request — the usual implementation asks a model to fix the
    /// arguments.
    func testRepairFunctionMayCallBackIntoAimux() throws {
        let server = MockHTTPServer(responses: [
            .json(openaiInvalidToolCallResponse),
            .json(openaiRepairSuggestionResponse),
        ])
        try server.start()
        defer { server.stop() }

        let model = try Model.openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
        let result = try model.generateText(
            prompt: .text("weather in Singapore?"),
            options: weatherOptions { context in
                let fixed = try model.generateText(prompt: .text("fix these arguments"))
                return RawToolCall(toolCallId: context.toolCall.toolCallId,
                                   toolName: context.toolCall.toolName,
                                   input: fixed.text)
            })

        XCTAssertNotEqual(result.toolCalls[0].invalid, true)
        XCTAssertEqual(result.toolCalls[0].input["city"]?.stringValue, "Singapore")
    }

    // MARK: - streaming

    /// The final tool-call part is delivered repaired; the argument deltas that
    /// preceded it are forwarded verbatim, as the AI SDK does.
    func testStreamReplacesTheToolCallPartAndLeavesDeltasAlone() throws {
        let server = MockHTTPServer(response: .sse(sseBody(openaiStreamInvalidToolEvents)))
        try server.start()
        defer { server.stop() }

        var parts: [StreamPart] = []
        var streamError: (any Error)?
        try Model
            .openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
            .streamText(prompt: .text("weather in Singapore?"),
                        options: weatherOptions { context in
                            RawToolCall(toolCallId: context.toolCall.toolCallId,
                                        toolName: context.toolCall.toolName,
                                        input: #"{"city":"Singapore"}"#)
                        },
                        onPart: { parts.append($0) },
                        onDone: {},
                        onError: { streamError = $0 })
        XCTAssertNil(streamError)

        let deltas = parts.reduce(into: "") { text, part in
            if case .toolInputDelta(_, let delta, _) = part { text += delta }
        }
        XCTAssertEqual(deltas, #"{"town":"Singapore"}"#, "argument deltas are the provider's own")

        let toolCalls = parts.compactMap { part -> (JSONValue, Bool?)? in
            if case .toolCall(_, _, let input, _, _, _, _, let invalid, _) = part {
                return (input, invalid)
            }
            return nil
        }
        guard let (input, invalid) = toolCalls.first else {
            return XCTFail("stream produced no ToolCall part")
        }
        XCTAssertNotEqual(invalid, true)
        XCTAssertEqual(input["city"]?.stringValue, "Singapore")
    }

    /// The stream's repair function runs on the native callback thread's watch,
    /// but not on that thread: the FFI re-entrancy guard is thread-local, so a
    /// closure that asks a model to fix the arguments — the canonical AI SDK
    /// use — would otherwise come back as `AIMUX_E_FFI_REENTRANT_CALL` (204).
    func testStreamRepairFunctionMayCallBackIntoAimux() throws {
        let server = MockHTTPServer(responses: [
            .sse(sseBody(openaiStreamInvalidToolEvents)),
            .json(openaiRepairSuggestionResponse),
        ])
        try server.start()
        defer { server.stop() }

        let model = try Model.openai(apiKey: "test-key", modelId: "gpt-4o", baseUrl: server.baseURL)
        var parts: [StreamPart] = []
        var streamError: (any Error)?
        var repairError: (any Error)?
        model.streamText(prompt: .text("weather in Singapore?"),
                         options: weatherOptions { context in
                             do {
                                 let fixed = try model.generateText(prompt: .text("fix these arguments"))
                                 return RawToolCall(toolCallId: context.toolCall.toolCallId,
                                                    toolName: context.toolCall.toolName,
                                                    input: fixed.text)
                             } catch {
                                 repairError = error
                                 throw error
                             }
                         },
                         onPart: { parts.append($0) },
                         onDone: {},
                         onError: { streamError = $0 })

        XCTAssertNil(streamError)
        XCTAssertNil(repairError, "a nested generate call must not hit the re-entrancy guard")

        let deltas = parts.reduce(into: "") { text, part in
            if case .toolInputDelta(_, let delta, _) = part { text += delta }
        }
        XCTAssertEqual(deltas, #"{"town":"Singapore"}"#, "argument deltas are the provider's own")

        let toolCalls = parts.compactMap { part -> (JSONValue, Bool?)? in
            if case .toolCall(_, _, let input, _, _, _, _, let invalid, _) = part {
                return (input, invalid)
            }
            return nil
        }
        guard let (input, invalid) = toolCalls.first else {
            return XCTFail("stream produced no ToolCall part")
        }
        XCTAssertNotEqual(invalid, true)
        XCTAssertEqual(input["city"]?.stringValue, "Singapore")
    }

    // MARK: - the option itself

    /// The repair function is host-side only: it must never reach the library
    /// as part of the options JSON.
    func testRepairFunctionIsNotSerialized() throws {
        let options = weatherOptions { _ in nil }
        let encoded = try JSONEncoder().encode(options)
        let wire = try JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        XCTAssertNotNil(wire?["tools"])
        XCTAssertNil(wire?["repair_tool_call"])
        XCTAssertNil(wire?["repairToolCall"])
        XCTAssertNil(wire?["repairToolCallBox"])

        // And it survives a decode as `nil` rather than failing the decode.
        let decoded = try JSONDecoder().decode(GenerateTextOptions.self, from: encoded)
        XCTAssertNil(decoded.repairToolCall)
    }
}

// MARK: - Helpers

/// `GenerateTextOptions` with the one-tool set every case here shares.
private func weatherOptions(_ repair: @escaping RepairToolCall) -> GenerateTextOptions {
    GenerateTextOptions(
        tools: [.function(FunctionTool(
            name: "weather",
            inputSchema: try! JSONDecoder().decode(
                JSONValue.self,
                from: Data(#"{"type":"object","properties":{"city":{"type":"string"}},"required":["city"],"additionalProperties":false}"#.utf8)
            )
        ))],
        repairToolCall: repair
    )
}

/// The `input` of the single tool-call part in a replayed transcript.
private func transcriptToolCallInput(_ messages: [ModelMessage]) -> JSONValue? {
    for message in messages {
        guard case .parts(let parts) = message.content else { continue }
        for part in parts {
            if case .toolCall(_, _, let input, _, _, _) = part { return input }
        }
    }
    return nil
}

/// Serialize a JSON value read out of the fixture back to a string.
private func json(_ value: Any) -> String {
    let data = try! JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
    return String(data: data, encoding: .utf8)!
}

private func assertJSONEqual(_ actual: String, _ expected: Any, case name: String,
                             file: StaticString = #filePath, line: UInt = #line) {
    let parsed = try? JSONSerialization.jsonObject(with: Data(actual.utf8), options: [.fragmentsAllowed])
    XCTAssertTrue((parsed as AnyObject).isEqual(expected as AnyObject),
                  "case '\(name)':\n  actual:   \(actual)\n  expected: \(json(expected))",
                  file: file, line: line)
}

private func sseBody(_ events: [[String: Any]]) -> String {
    events.map { "data: " + json($0) }.joined(separator: "\n\n") + "\n\ndata: [DONE]\n\n"
}

// MARK: - Mock provider responses

/// Arguments the `weather` schema rejects (`town` is not `city`).
private let openaiInvalidToolCallResponse: [String: Any] = [
    "id": "chatcmpl-invalid", "model": "gpt-4o",
    "choices": [[
        "message": [
            "role": "assistant", "content": NSNull(),
            "tool_calls": [[
                "id": "call-1", "type": "function",
                "function": ["name": "weather", "arguments": #"{"town":"Singapore"}"#],
            ]],
        ],
        "finish_reason": "tool_calls",
    ]],
    "usage": ["prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30],
]

private let openaiValidToolCallResponse: [String: Any] = [
    "id": "chatcmpl-valid", "model": "gpt-4o",
    "choices": [[
        "message": [
            "role": "assistant", "content": NSNull(),
            "tool_calls": [[
                "id": "call-1", "type": "function",
                "function": ["name": "weather", "arguments": #"{"city":"Singapore"}"#],
            ]],
        ],
        "finish_reason": "tool_calls",
    ]],
    "usage": ["prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30],
]

/// What the second (repair) model call answers: corrected argument text.
private let openaiRepairSuggestionResponse: [String: Any] = [
    "id": "chatcmpl-repair", "model": "gpt-4o",
    "choices": [[
        "message": ["role": "assistant", "content": #"{"city":"Singapore"}"#],
        "finish_reason": "stop",
    ]],
    "usage": ["prompt_tokens": 5, "completion_tokens": 5, "total_tokens": 10],
]

/// Streamed argument deltas that assemble into the same rejected arguments.
private let openaiStreamInvalidToolEvents: [[String: Any]] = [
    ["id": "1", "model": "gpt-4o", "choices": [["delta": [
        "role": "assistant",
        "tool_calls": [["index": 0, "id": "call-1", "type": "function",
                        "function": ["name": "weather", "arguments": ""]]],
    ]]]],
    ["id": "1", "model": "gpt-4o", "choices": [["delta": [
        "tool_calls": [["index": 0, "function": ["arguments": #"{"town":"#]]],
    ]]]],
    ["id": "1", "model": "gpt-4o", "choices": [["delta": [
        "tool_calls": [["index": 0, "function": ["arguments": #""Singapore"}"#]]],
    ]]]],
    ["id": "1", "model": "gpt-4o",
     "choices": [["delta": [String: Any](), "finish_reason": "tool_calls"]],
     "usage": ["prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7]],
]
