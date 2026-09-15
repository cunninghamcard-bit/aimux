package ai.arcships.aimux

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

// ─────────────────────────────────────────────────────────────────────────────
// Host `repairToolCall` (AI SDK) over the C ABI, against the shared
// [MockProviderServer]: the model drops the closing brace of the arguments, so
// Core hands the invalid tool call to the Kotlin repair function.
// ─────────────────────────────────────────────────────────────────────────────

class ToolCallRepairTest {

    private lateinit var server: MockProviderServer

    @BeforeEach
    fun setUp() {
        server = MockProviderServer()
        server.responseBody = malformedToolCallResponse
    }

    @AfterEach
    fun tearDown() {
        server.stop()
    }

    /** OpenAI tool call whose `arguments` are missing the closing brace. */
    private val malformedToolCallResponse: String = """
        {
          "id": "chatcmpl-bad",
          "model": "gpt-4o",
          "choices": [{
            "message": {
              "role": "assistant",
              "content": null,
              "tool_calls": [{
                "id": "call_bad",
                "type": "function",
                "function": {"name": "get_weather", "arguments": "{\"location\":\"Tokyo\""}
              }]
            },
            "finish_reason": "tool_calls"
          }],
          "usage": {"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30}
        }
    """.trimIndent()

    private val weatherTool: Tool = Tool.Function(
        name = "get_weather",
        inputSchema = JsonObject(
            mapOf(
                "type" to JsonPrimitive("object"),
                "properties" to JsonObject(
                    mapOf("location" to JsonObject(mapOf("type" to JsonPrimitive("string"))))
                ),
                "required" to AimuxJson.parseToJsonElement("""["location"]"""),
            )
        ),
    )

    /** One generateText against the mock server; returns its single tool call. */
    private fun generateWithRepair(repair: ToolCallRepair): ToolCall =
        TypedModel.openai("sk-test-fake-key", "gpt-4o", server.baseUrl).use { model ->
            val result = model.generateText(
                "What is the weather in Tokyo?",
                GenerateTextOptions(tools = listOf(weatherTool), repairToolCall = repair),
            )
            assertThat(result.toolCalls).hasSize(1)
            result.toolCalls[0]
        }

    @Test
    fun `a Kotlin repair function fixes malformed arguments`() {
        var calls = 0
        var seen: ToolCallRepairContext? = null
        ToolCallRepair { context ->
            calls++
            seen = context
            context.toolCall.copy(input = context.toolCall.input + "}")
        }.use { repair ->
            val call = generateWithRepair(repair)

            assertThat(calls).isEqualTo(1)
            assertThat(call.invalid).isNotEqualTo(true)
            assertThat(call.input.jsonObject["location"]!!.jsonPrimitive.content).isEqualTo("Tokyo")

            // The context carries the AI SDK repairToolCall arguments.
            val context = seen!!
            assertThat(context.toolCall.toolName).isEqualTo("get_weather")
            assertThat(context.toolCall.toolCallId).isEqualTo("call_bad")
            assertThat(context.error!!.jsonObject).containsKey("InvalidToolInput")
            assertThat(context.inputSchema.jsonObject["properties"]).isNotNull()
            assertThat(context.tools).hasSize(1)
            assertThat(context.messages).isNotEmpty()
        }
    }

    @Test
    fun `returning null keeps the original error and the raw text`() {
        ToolCallRepair { null }.use { repair ->
            val call = generateWithRepair(repair)

            assertThat(call.invalid).isTrue()
            assertThat(call.error).isNotNull()
            assertThat(call.input.jsonPrimitive.content).isEqualTo("""{"location":"Tokyo"""")
        }
    }

    @Test
    fun `a throwing repair function becomes a ToolCallRepair error`() {
        ToolCallRepair { throw IllegalArgumentException("no idea how to fix that") }.use { repair ->
            val call = generateWithRepair(repair)

            assertThat(call.invalid).isTrue()
            val failure = call.error!!.jsonObject["ToolCallRepair"]!!.jsonObject
            assertThat(failure["original_error"].toString()).contains("InvalidToolInput")
            assertThat(failure["cause"].toString()).contains("no idea how to fix that")
        }
    }

    @Test
    fun `calling back into aimux from the repair function is rejected as re-entrant`() {
        Model.openai("sk-test-fake-key", "gpt-4o", server.baseUrl).use { raw ->
            ToolCallRepair {
                // Same thread, inside the FFI re-entrancy guard: 204, which
                // throws here and reaches Core as the repair failure.
                raw.generateText("\"hi\"")
                null
            }.use { repair ->
                val result = TypedModel(raw).generateText(
                    "What is the weather in Tokyo?",
                    GenerateTextOptions(tools = listOf(weatherTool), repairToolCall = repair),
                )
                val call = result.toolCalls[0]
                assertThat(call.invalid).isTrue()
                assertThat(call.error!!.jsonObject["ToolCallRepair"]!!.jsonObject["cause"].toString())
                    .contains("re-entrant")
            }
        }
    }

    @Test
    fun `a closed repair serializes as null and closing twice is a no-op`() {
        val repair = ToolCallRepair { null }
        val open = AimuxJson.encodeToString(
            GenerateTextOptions.serializer(), GenerateTextOptions(repairToolCall = repair)
        )
        assertThat(open).matches("""\{"repair_tool_call":[1-9][0-9]*}""")

        repair.close()
        repair.close()
        assertThat(
            AimuxJson.encodeToString(
                GenerateTextOptions.serializer(), GenerateTextOptions(repairToolCall = repair)
            )
        ).isEqualTo("""{"repair_tool_call":null}""")
    }
}
