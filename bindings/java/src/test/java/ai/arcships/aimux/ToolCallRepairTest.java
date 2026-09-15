package ai.arcships.aimux;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.sun.jna.Pointer;
import com.sun.jna.ptr.PointerByReference;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Collections;
import java.util.concurrent.atomic.AtomicInteger;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * {@link ToolCallRepair} against the mock provider server (mirror of the Go
 * binding's repair_test.go): the model emits tool-call arguments with the
 * closing brace missing, and the host function gets one attempt to fix them.
 */
class ToolCallRepairTest {

    private MockProviderServer server;

    @BeforeEach
    void setUp() {
        server = new MockProviderServer();
    }

    @AfterEach
    void tearDown() {
        server.close();
    }

    /** The model drops the closing brace of the arguments. */
    private static String malformedToolCallResponse() {
        return new JSONObject()
            .put("id", "chatcmpl-bad")
            .put("model", "gpt-4o")
            .put("choices", new JSONArray().put(
                new JSONObject()
                    .put("message", new JSONObject()
                        .put("role", "assistant")
                        .put("content", JSONObject.NULL)
                        .put("tool_calls", new JSONArray().put(
                            new JSONObject()
                                .put("id", "call_bad")
                                .put("type", "function")
                                .put("function", new JSONObject()
                                    .put("name", "get_weather")
                                    .put("arguments", "{\"location\":\"Tokyo\"")))))
                    .put("finish_reason", "tool_calls")))
            .put("usage", new JSONObject()
                .put("prompt_tokens", 20)
                .put("completion_tokens", 10)
                .put("total_tokens", 30))
            .toString();
    }

    private static Types.Tool weatherTool() {
        ObjectMapper m = new ObjectMapper();
        ObjectNode schema = m.createObjectNode();
        schema.put("type", "object");
        ObjectNode props = m.createObjectNode();
        props.set("location", m.createObjectNode().put("type", "string"));
        schema.set("properties", props);
        schema.set("required", m.createArrayNode().add("location"));
        return Types.Tool.Function.builder().name("get_weather").inputSchema(schema).build();
    }

    /** Generate against the malformed response with {@code repair} registered. */
    private Types.ToolCall generateWithRepair(ToolCallRepair repair) {
        server.setResponseBody(malformedToolCallResponse());
        Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
            .tools(Collections.singletonList(weatherTool()))
            .repairToolCall(repair)
            .build();
        try (TypedModel model =
                 TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl())) {
            Types.GenerateTextResult result =
                model.generateText("What is the weather in Tokyo?", options);
            assertThat(result.getToolCalls()).hasSize(1);
            return result.getToolCalls().get(0);
        }
    }

    @Test
    void hostFunctionFixesMalformedArguments() {
        AtomicInteger calls = new AtomicInteger();
        try (ToolCallRepair repair = new ToolCallRepair(context -> {
            calls.incrementAndGet();
            assertThat(context.getToolCall().getToolName()).isEqualTo("get_weather");
            assertThat(context.getTools()).hasSize(1);
            assertThat(context.getInputSchema().path("type").asText()).isEqualTo("object");
            assertThat(context.getError().has("InvalidToolInput")).isTrue();
            assertThat(context.getMessages()).isNotEmpty();
            return context.getToolCall().withInput(context.getToolCall().getInput() + "}");
        })) {
            Types.ToolCall call = generateWithRepair(repair);

            assertThat(calls.get()).isEqualTo(1);
            assertThat(call.getInvalid()).isNull();
            assertThat(call.getInput().path("location").asText()).isEqualTo("Tokyo");
            // Also the guard for the assertions above: the callback swallows
            // every Throwable, so a failed one only shows up here.
            assertThat(repair.lastError()).isNull();
        }
    }

    @Test
    void nullKeepsTheOriginalError() {
        try (ToolCallRepair repair = new ToolCallRepair(context -> null)) {
            Types.ToolCall call = generateWithRepair(repair);

            assertThat(call.getInvalid()).isTrue();
            assertThat(call.getError().has("InvalidToolInput")).isTrue();
            // The raw argument text survives as-is for the caller to inspect.
            assertThat(call.getInput().asText()).isEqualTo("{\"location\":\"Tokyo\"");
            assertThat(repair.lastError()).isNull();
        }
    }

    @Test
    void thrownExceptionIsKeptOnTheRepair() {
        final IllegalStateException boom = new IllegalStateException("boom");
        try (ToolCallRepair repair = new ToolCallRepair(context -> {
            throw boom;
        })) {
            Types.ToolCall call = generateWithRepair(repair);

            assertThat(call.getInvalid()).isTrue();
            assertThat(repair.lastError()).isSameAs(boom);
        }
    }

    /**
     * The repair function runs inside the FFI re-entrancy guard, so an
     * {@code aimux_*} call from within it fails with
     * {@code AIMUX_E_FFI_REENTRANT_CALL} (204) instead of deadlocking.
     */
    @Test
    void callingBackIntoAimuxIsRejectedAsReentrant() {
        final AtomicInteger innerCode = new AtomicInteger();
        try (Model inner =
                 Model.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl());
             ToolCallRepair repair = new ToolCallRepair(context -> {
                 // The raw FFI call, so the C code is observable (the Java
                 // layer collapses 200–206 into IllegalStateException).
                 PointerByReference out = new PointerByReference();
                 Pointer err = AimuxFFI.INSTANCE.aimux_generate_text(
                     inner.handle(), "\"hi\"", null, out);
                 innerCode.set(AimuxFFI.INSTANCE.aimux_error_code(err));
                 AimuxFFI.INSTANCE.aimux_error_free(err);
                 return null;
             })) {
            Types.ToolCall call = generateWithRepair(repair);

            assertThat(innerCode.get()).isEqualTo(204);
            assertThat(call.getInvalid()).isTrue();
            assertThat(repair.lastError()).isNull();
        }
    }

    @Test
    void closedRepairSerializesAsNull() throws Exception {
        ToolCallRepair repair = new ToolCallRepair(context -> null);
        repair.close();
        repair.close(); // idempotent

        String json = Types.AimuxJson.MAPPER.writeValueAsString(
            Types.GenerateTextOptions.builder().repairToolCall(repair).build());
        assertThat(json).isEqualTo("{\"repair_tool_call\":null}");
    }
}
