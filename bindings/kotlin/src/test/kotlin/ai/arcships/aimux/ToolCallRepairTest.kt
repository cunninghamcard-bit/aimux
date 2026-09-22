package ai.arcships.aimux

import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.assertj.core.api.Assertions.assertThat
import org.assertj.core.api.Assertions.assertThatThrownBy
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertTimeoutPreemptively
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import java.io.File
import java.time.Duration
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

// ─────────────────────────────────────────────────────────────────────────────
// Host-side tool-call repair (RFC-0035).
//
// Two halves:
//   1. the shared `tool-call-repair.json` fixture replayed through the three
//      native entry points (pure functions — no model, no server),
//   2. end-to-end through TypedModel against MockProviderServer: the provider
//      returns arguments that violate the tool schema, Core marks the call
//      invalid, and the repair function runs on the JVM.
// ─────────────────────────────────────────────────────────────────────────────

class ToolCallRepairTest {

    private lateinit var server: MockProviderServer

    @BeforeEach
    fun setUp() {
        server = MockProviderServer()
    }

    @AfterEach
    fun tearDown() {
        server.stop()
    }

    // ── shared fixture ──────────────────────────────────────────────────

    private val fixtureCandidates = listOf(
        "../../contract-tests/fixtures/tool-call-repair.json", // gradle test cwd = bindings/kotlin
        "contract-tests/fixtures/tool-call-repair.json",
        "../contract-tests/fixtures/tool-call-repair.json",
    )

    private fun fixtureCase(name: String): JsonObject {
        val file = fixtureCandidates.map(::File).firstOrNull { it.isFile }
            ?: throw IllegalStateException(
                "cannot find tool-call-repair.json; tried $fixtureCandidates from ${File(".").absolutePath}"
            )
        val cases = AimuxJson.parseToJsonElement(file.readText()).jsonObject["cases"]!!
        return cases.let { it as kotlinx.serialization.json.JsonArray }
            .map { it.jsonObject }
            .firstOrNull { it["name"]?.jsonPrimitive?.content == name }
            ?: throw IllegalStateException("no fixture case named '$name'")
    }

    /** `opts` may be JSON null in the fixture, which is the binding's "defaults". */
    private fun optsOf(input: JsonObject): String? =
        input["opts"]?.takeIf { it !is JsonNull }?.toString()

    @Test
    fun `context fixture cases replay through the native function`() {
        for (name in listOf("context_string_prompt", "context_messages_prompt", "context_prompt_wrapper")) {
            val case = fixtureCase(name)
            val input = case["input"]!!.jsonObject
            val actual = toolCallRepairContext(
                input["tool_call"]!!.toString(),
                input["prompt"]!!.toString(),
                optsOf(input),
            )
            assertThat(AimuxJson.parseToJsonElement(actual))
                .describedAs(name)
                .isEqualTo(case["expected"]!!)
        }
    }

    @Test
    fun `a call made without a tool set yields null - the skip signal`() {
        val input = fixtureCase("context_without_tools")["input"]!!.jsonObject
        val actual = toolCallRepairContext(
            input["tool_call"]!!.toString(),
            input["prompt"]!!.toString(),
            optsOf(input),
        )
        assertThat(AimuxJson.parseToJsonElement(actual)).isEqualTo(JsonNull)
    }

    @Test
    fun `apply fixture cases replay through the native function`() {
        for (name in listOf(
            "reply_repaired_validates",
            "reply_repaired_still_invalid",
            "reply_unchanged",
            "reply_failed",
        )) {
            val input = fixtureCase(name)["input"]!!.jsonObject
            val actual = applyToolCallRepair(
                input["tool_call"]!!.toString(),
                optsOf(input),
                input["reply"]!!.toString(),
            )
            assertThat(AimuxJson.parseToJsonElement(actual))
                .describedAs(name)
                .isEqualTo(fixtureCase(name)["expected"]!!)
        }
    }

    @Test
    fun `patch result fixture case replays through the native function`() {
        val case = fixtureCase("patch_result")
        val input = case["input"]!!.jsonObject
        val actual = applyToolCallRepairToResult(
            input["result"]!!.toString(),
            optsOf(input),
            input["tool_call_id"]!!.jsonPrimitive.content,
            input["reply"]!!.toString(),
        )
        assertThat(AimuxJson.parseToJsonElement(actual)).isEqualTo(case["expected"]!!)
    }

    @Test
    fun `patching an unknown tool call id is an InvalidArgument, not a silent no-op`() {
        val input = fixtureCase("patch_result_unknown_id")["input"]!!.jsonObject
        assertThatThrownBy {
            applyToolCallRepairToResult(
                input["result"]!!.toString(),
                optsOf(input),
                input["tool_call_id"]!!.jsonPrimitive.content,
                input["reply"]!!.toString(),
            )
        }.isInstanceOf(InvalidArgumentError::class.java)
    }

    // ── end to end ──────────────────────────────────────────────────────

    /** `city` is required; the canned response sends `town` instead. */
    private val weatherTool: Tool = Tool.Function(
        name = "weather",
        inputSchema = AimuxJson.parseToJsonElement(
            """{"type":"object","properties":{"city":{"type":"string"}},"required":["city"],"additionalProperties":false}"""
        ),
    )

    private fun openAiToolCall(arguments: String, name: String = "weather"): String =
        """
        {"id":"chatcmpl-1","model":"gpt-4o","choices":[{"message":{"role":"assistant","content":null,
         "tool_calls":[{"id":"call-1","type":"function","function":{"name":"$name","arguments":${
            JsonPrimitive(arguments)
        }}}]},"finish_reason":"tool_calls"}],
         "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}
        """.trimIndent()

    private fun model(): TypedModel = TypedModel.openai("sk-test-fake-key", "gpt-4o", server.baseUrl)

    private fun options(repair: RepairToolCall?): GenerateTextOptions =
        GenerateTextOptions(tools = listOf(weatherTool), repairToolCall = repair)

    @Test
    fun `a repaired call becomes valid in both tool_calls and response_messages`() {
        server.responseBody = openAiToolCall("""{"town":"Singapore"}""")

        model().use { model ->
            val result = model.generateText(
                "weather in Singapore?",
                options { context ->
                    // The repair function sees the RAW argument text, not a parsed object.
                    assertThat(context.toolCall.input).isEqualTo("""{"town":"Singapore"}""")
                    assertThat(context.inputSchema.jsonObject["required"].toString()).contains("city")
                    assertThat(context.messages).hasSize(1)
                    context.toolCall.copy(input = """{"city":"Singapore"}""")
                },
            )

            val call = result.toolCalls.single()
            assertThat(call.invalid).isNotEqualTo(true)
            assertThat(call.error).isNull()
            assertThat(call.input.jsonObject["city"]?.jsonPrimitive?.content).isEqualTo("Singapore")

            // The transcript must agree, or the next turn replays bad arguments.
            val part = result.responseMessages
                .flatMap { it.contentParts ?: emptyList() }
                .filterIsInstance<ContentPart.ToolCall>()
                .single()
            assertThat(part.toolCallId).isEqualTo("call-1")
            assertThat(part.input.jsonObject["city"]?.jsonPrimitive?.content).isEqualTo("Singapore")
        }
    }

    @Test
    fun `returning null keeps the invalid call and its original error`() {
        server.responseBody = openAiToolCall("""{"town":"Singapore"}""")

        model().use { model ->
            val result = model.generateText("weather in Singapore?", options { null })
            val call = result.toolCalls.single()
            assertThat(call.invalid).isTrue()
            assertThat(call.error!!.jsonObject.keys).containsExactly("InvalidToolInput")
        }
    }

    @Test
    fun `a throwing repair function becomes a ToolCallRepair error carrying its message`() {
        server.responseBody = openAiToolCall("""{"town":"Singapore"}""")

        model().use { model ->
            val result = model.generateText(
                "weather in Singapore?",
                options { throw IllegalStateException("repair model unavailable") },
            )
            val call = result.toolCalls.single()
            assertThat(call.invalid).isTrue()
            val repair = call.error!!.jsonObject["ToolCallRepair"]!!.jsonObject
            assertThat(repair.toString()).contains("repair model unavailable")
        }
    }

    @Test
    fun `a repair that is still invalid reports ToolCallRepair, not the original error`() {
        server.responseBody = openAiToolCall("""{"town":"Singapore"}""")

        model().use { model ->
            val result = model.generateText(
                "weather in Singapore?",
                options { it.toolCall.copy(input = """{"village":"Singapore"}""") },
            )
            val call = result.toolCalls.single()
            assertThat(call.invalid).isTrue()
            assertThat(call.error!!.jsonObject.keys).containsExactly("ToolCallRepair")
        }
    }

    @Test
    fun `a valid call never reaches the repair function`() {
        server.responseBody = openAiToolCall("""{"city":"Singapore"}""")
        var invoked = false

        model().use { model ->
            val result = model.generateText("weather in Singapore?", options { invoked = true; null })
            assertThat(result.toolCalls.single().invalid).isNotEqualTo(true)
        }
        assertThat(invoked).isFalse()
    }

    @Test
    fun `without a tool set the repair function is never invoked`() {
        server.responseBody = openAiToolCall("""{"town":"Singapore"}""")
        var invoked = false

        model().use { model ->
            // No `tools`: AI SDK never repairs such a call, and Core says so by
            // writing null for the context.
            model.generateText(
                "weather in Singapore?",
                GenerateTextOptions(repairToolCall = { invoked = true; null }),
            )
        }
        assertThat(invoked).isFalse()
    }

    @Test
    fun `the repair function may call back into aimux`() {
        server.setResponses(
            openAiToolCall("""{"town":"Singapore"}"""),
            // The nested generation the repair function performs.
            """{"id":"chatcmpl-2","model":"gpt-4o","choices":[{"message":{"role":"assistant",
               "content":"{\"city\":\"Singapore\"}"},"finish_reason":"stop"}],
               "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}""",
        )

        model().use { model ->
            val result = model.generateText(
                "weather in Singapore?",
                options { context ->
                    // Runs outside any native call, so a second FFI call is safe.
                    val fixed = model.generateText("fix ${context.toolCall.input}")
                    context.toolCall.copy(input = fixed.text)
                },
            )
            assertThat(result.toolCalls.single().input.jsonObject["city"]?.jsonPrimitive?.content)
                .isEqualTo("Singapore")
        }
    }

    /** The nested generation a repair hook performs: valid arguments as text. */
    private val repairCompletion: String =
        """{"id":"chatcmpl-2","model":"gpt-4o","choices":[{"message":{"role":"assistant",
           "content":"{\"city\":\"Singapore\"}"},"finish_reason":"stop"}],
           "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"""

    /** SSE: tool-call start, one arguments delta violating the schema, finish. */
    private val weatherSse: String = buildString {
        append("""data: {"id":"1","model":"gpt-4o","choices":[{"delta":{"role":"assistant","tool_calls":""")
        append("""[{"index":0,"id":"call-1","type":"function","function":{"name":"weather","arguments":""}}]}}]}""")
        append("\n\n")
        append("""data: {"id":"1","model":"gpt-4o","choices":[{"delta":{"tool_calls":""")
        append("""[{"index":0,"function":{"arguments":"{\"town\":\"Singapore\"}"}}]}}]}""")
        append("\n\n")
        append("""data: {"id":"1","model":"gpt-4o","choices":[{"delta":{},"finish_reason":"tool_calls"}]}""")
        append("\n\ndata: [DONE]\n\n")
    }

    @Test
    fun `streaming replaces the tool-call part and leaves the deltas alone`() {
        server.contentType = "text/event-stream"
        server.responseBody = weatherSse

        model().use { model ->
            val parts = model.streamTextSequence(
                "weather in Singapore?",
                options { it.toolCall.copy(input = """{"city":"Singapore"}""") },
            ).toList()

            val call = parts.filterIsInstance<StreamPart.ToolCall>().single()
            assertThat(call.invalid).isNotEqualTo(true)
            assertThat(call.input.jsonObject["city"]?.jsonPrimitive?.content).isEqualTo("Singapore")

            // Deltas are the provider's text, forwarded untouched.
            val delta = parts.filterIsInstance<StreamPart.ToolInputDelta>().single()
            assertThat(delta.delta).isEqualTo("""{"town":"Singapore"}""")
        }
    }

    @Test
    fun `a streaming hook may call a model - it runs off the FFI callback thread`() {
        // The native re-entrancy guard is thread-local and set on the thread
        // that runs the stream callbacks: a hook calling generate from there
        // would come back as AIMUX_E_FFI_REENTRANT_CALL (204), which this
        // binding would report as a failed repair. A repaired call proves the
        // hook ran off that thread.
        server.contentType = "text/event-stream"
        server.responseBody = weatherSse

        // A second server: the streaming one is pinned to text/event-stream.
        val repairServer = MockProviderServer()
        repairServer.responseBody =
            """{"id":"chatcmpl-2","model":"gpt-4o","choices":[{"message":{"role":"assistant",
               "content":"{\"city\":\"Singapore\"}"},"finish_reason":"stop"}],
               "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"""

        try {
            TypedModel.openai("sk-test-fake-key", "gpt-4o", repairServer.baseUrl).use { repairModel ->
                model().use { model ->
                    val parts = model.streamTextSequence(
                        "weather in Singapore?",
                        options { context ->
                            val fixed = repairModel.generateText("Fix ${context.toolCall.input}")
                            context.toolCall.copy(input = fixed.text)
                        },
                    ).toList()

                    val call = parts.filterIsInstance<StreamPart.ToolCall>().single()
                    // A 204 would surface here as a ToolCallRepair error.
                    assertThat(call.error).isNull()
                    assertThat(call.invalid).isNotEqualTo(true)
                    assertThat(call.input.jsonObject["city"]?.jsonPrimitive?.content).isEqualTo("Singapore")

                    val delta = parts.filterIsInstance<StreamPart.ToolInputDelta>().single()
                    assertThat(delta.delta).isEqualTo("""{"town":"Singapore"}""")
                }
            }
        } finally {
            repairServer.stop()
        }
    }

    @Test
    fun `a streaming hook may call the SAME model while another thread closes it`() {
        // The lock is fair, so a reader that arrives after close() has queued
        // for the write lock waits for it. Without the lent read hold this is a
        // three-way deadlock: stream thread (holds read, waits for the hook) ←
        // closer (waits for the write lock) ← hook (waits for a read lock).
        server.contentType = "text/event-stream"
        server.setResponses(weatherSse, repairCompletion)

        val model = model()
        val hookStarted = CountDownLatch(1)
        val closer = Thread {
            hookStarted.await()
            model.close()
        }
        closer.isDaemon = true
        // assertTimeoutPreemptively, not @Timeout: a regression deadlocks, and
        // the default @Timeout mode only checks after the body returns.
        assertTimeoutPreemptively(Duration.ofSeconds(30)) {
            closer.start()
            val parts = model.streamTextSequence(
                "weather in Singapore?",
                options { context ->
                    // The stream response is already fully written; the nested
                    // completion is plain JSON off the same one-server queue.
                    server.contentType = "application/json"
                    hookStarted.countDown()
                    // Let close() reach the write lock and queue ahead of this
                    // thread's read acquisition — that ordering is the bug, and
                    // without the sleep the hook wins the race and proves
                    // nothing.
                    Thread.sleep(250)
                    val fixed = model.generateText("Fix ${context.toolCall.input}")
                    context.toolCall.copy(input = fixed.text)
                },
            ).toList()

            val call = parts.filterIsInstance<StreamPart.ToolCall>().single()
            assertThat(call.error).isNull()
            assertThat(call.invalid).isNotEqualTo(true)
            assertThat(call.input.jsonObject["city"]?.jsonPrimitive?.content).isEqualTo("Singapore")

            closer.join(TimeUnit.SECONDS.toMillis(10))
            assertThat(closer.isAlive).describedAs("close() never completed").isFalse()
        }
    }

    @Test
    fun `the repair function is never serialized into the options JSON`() {
        val encoded = AimuxJson.encodeToString(
            GenerateTextOptions.serializer(),
            options { null },
        )
        assertThat(encoded).doesNotContain("repair")
    }
}
