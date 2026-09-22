package ai.arcships.aimux;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.io.IOException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.stream.Collectors;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.junit.jupiter.api.Assertions.assertTimeoutPreemptively;

/**
 * Host-side tool-call repair (RFC-0035).
 *
 * <p>Two layers: a replay of the shared
 * {@code contract-tests/fixtures/tool-call-repair.json} over the three native
 * functions, and the binding's own loops ({@link ToolCallRepairs#repairResult}
 * / {@link ToolCallRepairs#repairStreamPart}) that {@link TypedModel} drives.
 * Both need the native library on {@code java.library.path}; none of them
 * needs a provider — repair is pure post-processing.
 */
class ToolCallRepairTest {

    private static final String[] FIXTURE_CANDIDATES = {
        "../../contract-tests/fixtures/tool-call-repair.json", // gradle test cwd = bindings/java
        "contract-tests/fixtures/tool-call-repair.json",       // run from the repo root
        "../contract-tests/fixtures/tool-call-repair.json",
    };

    // ── shared fixture replay ────────────────────────────────────────────────

    @Test
    void contractFixtureReplays() throws Exception {
        JsonNode cases = loadFixture().get("cases");
        List<String> replayed = new ArrayList<>();
        for (JsonNode c : cases) {
            String name = c.get("name").asText();
            String expectedError = c.path("expected_error").asText(null);
            if (expectedError != null) {
                // Every error case in the fixture is InvalidArgument; if a new
                // variant appears, this pins it rather than silently passing.
                assertThat(expectedError).describedAs(name).isEqualTo("InvalidArgument");
                assertThatThrownBy(() -> invoke(c))
                    .describedAs(name)
                    .isInstanceOf(AimuxException.InvalidArgumentError.class);
            } else {
                assertThat(json(invoke(c))).describedAs(name).isEqualTo(c.get("expected"));
            }
            replayed.add(name);
        }
        assertThat(replayed).hasSize(cases.size());
    }

    private static String invoke(JsonNode testCase) {
        JsonNode in = testCase.get("input");
        String function = testCase.get("function").asText();
        String opts = text(in.get("opts"));
        if ("tool_call_repair_context".equals(function)) {
            return ToolCallRepairs.context(in.get("tool_call").toString(), text(in.get("prompt")), opts);
        }
        if ("apply_tool_call_repair".equals(function)) {
            return ToolCallRepairs.apply(in.get("tool_call").toString(), opts, in.get("reply").toString());
        }
        return ToolCallRepairs.applyToResult(in.get("result").toString(), opts,
            in.get("tool_call_id").asText(), in.get("reply").toString());
    }

    // ── the non-streaming loop ───────────────────────────────────────────────

    @Test
    void repairedCallPatchesToolCallsAndResponseMessages() throws Exception {
        JsonNode patched = json(ToolCallRepairs.repairResult(
            result(), PROMPT, opts(true), context -> {
                assertThat(context.getToolCall().getInput()).isEqualTo("{\"town\":\"Singapore\"}");
                assertThat(context.getToolCall().getToolName()).isEqualTo("weather");
                assertThat(context.getError().path("InvalidToolInput").path("tool_name").asText())
                    .isEqualTo("weather");
                assertThat(context.getInputSchema().path("required").get(0).asText()).isEqualTo("city");
                assertThat(context.getTools()).hasSize(1);
                assertThat(context.getMessages()).hasSize(1);
                assertThat(context.getInstructions()).isEqualTo("be terse");
                return Types.RawToolCall.builder()
                    .toolCallId("call-1").toolName("weather").input("{\"city\":\"Singapore\"}").build();
            }));

        JsonNode call = patched.get("tool_calls").get(0);
        assertThat(call.path("invalid").isMissingNode()).isTrue();
        assertThat(call.path("error").isMissingNode()).isTrue();
        assertThat(call.get("input").get("city").asText()).isEqualTo("Singapore");
        // The transcript must agree, or the next turn replays the broken arguments.
        JsonNode part = patched.get("response_messages").get(0).get("content").get(1);
        assertThat(part.get("input").get("city").asText()).isEqualTo("Singapore");
    }

    @Test
    void nullReturnLeavesTheCallInvalid() throws Exception {
        JsonNode call = json(ToolCallRepairs.repairResult(result(), PROMPT, opts(true), context -> null))
            .get("tool_calls").get(0);
        assertThat(call.get("invalid").asBoolean()).isTrue();
        assertThat(call.get("error").has("InvalidToolInput")).isTrue();
        assertThat(call.get("input").get("town").asText()).isEqualTo("Singapore");
    }

    @Test
    void thrownExceptionBecomesAToolCallRepairError() throws Exception {
        JsonNode call = json(ToolCallRepairs.repairResult(result(), PROMPT, opts(true), context -> {
            throw new IllegalStateException("repair model unavailable");
        })).get("tool_calls").get(0);

        JsonNode error = call.get("error").get("ToolCallRepair");
        assertThat(call.get("invalid").asBoolean()).isTrue();
        assertThat(error.get("original_error").has("InvalidToolInput")).isTrue();
        assertThat(error.get("cause").get("Other").asText()).isEqualTo("repair model unavailable");
    }

    @Test
    void replacementThatStillFailsValidationReportsBothErrors() throws Exception {
        JsonNode call = json(ToolCallRepairs.repairResult(result(), PROMPT, opts(true), context ->
            Types.RawToolCall.builder()
                .toolCallId("call-1").toolName("weather").input("{\"city\":42}").build()))
            .get("tool_calls").get(0);

        JsonNode error = call.get("error").get("ToolCallRepair");
        assertThat(call.get("invalid").asBoolean()).isTrue();
        assertThat(call.get("input").get("town").asText()).isEqualTo("Singapore"); // original kept
        assertThat(error.get("original_error").has("InvalidToolInput")).isTrue();
        assertThat(error.get("cause").path("InvalidToolInput").path("tool_input").asText())
            .isEqualTo("{\"city\":42}");
    }

    @Test
    void withoutAToolSetTheHookIsNeverInvoked() throws Exception {
        AtomicInteger calls = new AtomicInteger();
        String patched = ToolCallRepairs.repairResult(result(), PROMPT, opts(false), context -> {
            calls.incrementAndGet();
            return null;
        });
        assertThat(calls.get()).isZero();
        assertThat(json(patched)).isEqualTo(json(result()));
    }

    @Test
    void aValidCallNeverReachesTheHook() throws Exception {
        AtomicInteger calls = new AtomicInteger();
        String valid = "{\"text\":\"\",\"tool_calls\":[{\"tool_call_id\":\"call-1\","
            + "\"tool_name\":\"weather\",\"input\":{\"city\":\"Singapore\"}}]}";
        String patched = ToolCallRepairs.repairResult(valid, PROMPT, opts(true), context -> {
            calls.incrementAndGet();
            return null;
        });
        assertThat(calls.get()).isZero();
        assertThat(patched).isEqualTo(valid);
    }

    @Test
    void theHookMayCallBackIntoAimux() throws Exception {
        // The hook runs outside every native call, so re-entering the library
        // (here: the repair functions themselves; in practice a second
        // generateText against a repair model) just works.
        JsonNode call = json(ToolCallRepairs.repairResult(result(), PROMPT, opts(true), context -> {
            JsonNode nested = json(ToolCallRepairs.context(
                invalidCall(), PROMPT, opts(true)));
            return Types.RawToolCall.builder()
                .toolCallId("call-1")
                .toolName(nested.get("tool_call").get("tool_name").asText())
                .input("{\"city\":\"Singapore\"}")
                .build();
        })).get("tool_calls").get(0);
        assertThat(call.get("input").get("city").asText()).isEqualTo("Singapore");
    }

    @Test
    void objectResultsAreRepairedUnderRaw() throws Exception {
        String objectResult = "{\"object\":{\"ok\":true},\"raw\":" + result() + "}";
        JsonNode patched = json(ToolCallRepairs.repairResult(objectResult, PROMPT, opts(true), context ->
            Types.RawToolCall.builder()
                .toolCallId("call-1").toolName("weather").input("{\"city\":\"Singapore\"}").build()));
        assertThat(patched.get("raw").get("tool_calls").get(0).get("input").get("city").asText())
            .isEqualTo("Singapore");
        assertThat(patched.get("object").get("ok").asBoolean()).isTrue();
    }

    // ── the stream loop ──────────────────────────────────────────────────────

    @Test
    void streamToolCallPartIsReplacedAndDeltasPassThrough() throws Exception {
        String delta = "{\"ToolInputDelta\":{\"id\":\"call-1\",\"delta\":\"{\\\"town\\\":\"}}";
        assertThat(ToolCallRepairs.repairStreamPart(delta, PROMPT, opts(true), context -> {
            throw new AssertionError("deltas must never be repaired");
        }, null)).isSameAs(delta);

        String part = "{\"ToolCall\":" + invalidCall() + "}";
        JsonNode repaired = json(ToolCallRepairs.repairStreamPart(part, PROMPT, opts(true), context ->
            Types.RawToolCall.builder()
                .toolCallId("call-1").toolName("weather").input("{\"city\":\"Singapore\"}").build(),
            null));
        assertThat(repaired.get("ToolCall").get("input").get("city").asText()).isEqualTo("Singapore");
        assertThat(repaired.get("ToolCall").path("invalid").isMissingNode()).isTrue();

        // The decoded part is what a TypedModel consumer receives.
        Types.StreamPart decoded =
            Types.AimuxJson.MAPPER.readValue(repaired.toString(), Types.StreamPart.class);
        assertThat(((Types.StreamPart.ToolCall) decoded).getInput().get("city").asText())
            .isEqualTo("Singapore");
    }

    @Test
    void aValidStreamPartIsReturnedUntouched() {
        String part = "{\"ToolCall\":{\"tool_call_id\":\"call-1\",\"tool_name\":\"weather\","
            + "\"input\":{\"city\":\"Singapore\"}}}";
        assertThat(ToolCallRepairs.repairStreamPart(part, PROMPT, opts(true), context -> {
            throw new AssertionError("a valid call must never be repaired");
        }, null)).isSameAs(part);
    }

    // ── end to end through TypedModel (mock provider) ────────────────────────

    @Test
    void generateTextRepairsTheInvalidCallItGotBack() {
        try (MockProviderServer server = new MockProviderServer()) {
            server.setResponseBody(toolCallResponse("{\"city\":\"Tokyo\"}"));
            Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
                .tools(Collections.singletonList(strictWeatherTool()))
                .repairToolCall(context -> Types.RawToolCall.builder()
                    .toolCallId(context.getToolCall().getToolCallId())
                    .toolName(context.getToolCall().getToolName())
                    .input(context.getToolCall().getInput().replace("city", "location"))
                    .build())
                .build();

            try (TypedModel model =
                     TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl())) {
                Types.GenerateTextResult result =
                    model.generateText("What is the weather in Tokyo?", options);

                Types.ToolCall call = result.getToolCalls().get(0);
                assertThat(call.getInvalid()).isNull();
                assertThat(call.getInput().get("location").asText()).isEqualTo("Tokyo");
            }
        }
    }

    @Test
    void generateTextWithoutAHookKeepsTheInvalidCall() {
        try (MockProviderServer server = new MockProviderServer()) {
            server.setResponseBody(toolCallResponse("{\"city\":\"Tokyo\"}"));
            Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
                .tools(Collections.singletonList(strictWeatherTool()))
                .build();

            try (TypedModel model =
                     TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl())) {
                Types.ToolCall call = model.generateText("What is the weather in Tokyo?", options)
                    .getToolCalls().get(0);
                assertThat(call.getInvalid()).isTrue();
            }
        }
    }

    @Test
    void streamTextStreamDeliversTheRepairedToolCallPart() {
        try (MockProviderServer server = new MockProviderServer()) {
            server.setContentType("text/event-stream");
            server.setResponseBody(toolCallSse("{\"city\":\"Tokyo\"}"));
            Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
                .tools(Collections.singletonList(strictWeatherTool()))
                .repairToolCall(context -> Types.RawToolCall.builder()
                    .toolCallId(context.getToolCall().getToolCallId())
                    .toolName(context.getToolCall().getToolName())
                    .input(context.getToolCall().getInput().replace("city", "location"))
                    .build())
                .build();

            try (TypedModel model =
                     TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl())) {
                List<Types.StreamPart> parts =
                    model.streamTextStream("What is the weather in Tokyo?", options)
                        .collect(Collectors.toList());

                List<Types.StreamPart.ToolCall> calls = parts.stream()
                    .filter(p -> p instanceof Types.StreamPart.ToolCall)
                    .map(p -> (Types.StreamPart.ToolCall) p)
                    .collect(Collectors.toList());
                assertThat(calls).hasSize(1);
                assertThat(calls.get(0).getInvalid()).isNull();
                assertThat(calls.get(0).getInput().get("location").asText()).isEqualTo("Tokyo");

                // Argument deltas are forwarded verbatim, repair or not.
                assertThat(parts.stream()
                    .filter(p -> p instanceof Types.StreamPart.ToolInputDelta)
                    .map(p -> ((Types.StreamPart.ToolInputDelta) p).getDelta())
                    .collect(Collectors.joining())).contains("city");
            }
        }
    }

    @Test
    void aStreamHookMayGenerateWithAnotherModel() {
        // The canonical AI SDK repair: ask a model to fix the arguments. The FFI
        // re-entrancy guard is thread-local, so this only works if the hook runs
        // off the stream-callback thread (otherwise: AIMUX_E_FFI_REENTRANT_CALL).
        try (MockProviderServer streamServer = new MockProviderServer();
             MockProviderServer repairServer = new MockProviderServer()) {
            streamServer.setContentType("text/event-stream");
            streamServer.setResponseBody(toolCallSse("{\"city\":\"Tokyo\"}"));
            repairServer.setResponseBody(textResponse("{\"location\":\"Tokyo\"}"));

            try (TypedModel repairModel =
                     TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", repairServer.baseUrl());
                 TypedModel model =
                     TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", streamServer.baseUrl())) {

                Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
                    .tools(Collections.singletonList(strictWeatherTool()))
                    .repairToolCall(context -> Types.RawToolCall.builder()
                        .toolCallId(context.getToolCall().getToolCallId())
                        .toolName(context.getToolCall().getToolName())
                        .input(repairModel.generateText(
                            "fix these arguments: " + context.getToolCall().getInput()).getText())
                        .build())
                    .build();

                List<Types.StreamPart> parts = new ArrayList<>();
                model.streamText("What is the weather in Tokyo?", options,
                    parts::add, () -> {}, error -> { throw new AssertionError(error); });

                List<Types.StreamPart.ToolCall> calls = parts.stream()
                    .filter(p -> p instanceof Types.StreamPart.ToolCall)
                    .map(p -> (Types.StreamPart.ToolCall) p)
                    .collect(Collectors.toList());
                assertThat(calls).hasSize(1);
                assertThat(calls.get(0).getInvalid()).isNull();
                assertThat(calls.get(0).getInput().get("location").asText()).isEqualTo("Tokyo");
                assertThat(repairServer.lastRequestBody()).contains("fix these arguments");

                // Deltas stay the provider's own text.
                assertThat(parts.stream()
                    .filter(p -> p instanceof Types.StreamPart.ToolInputDelta)
                    .map(p -> ((Types.StreamPart.ToolInputDelta) p).getDelta())
                    .collect(Collectors.joining())).contains("city");
            }
        }
    }

    @Test
    void aStreamHookMayGenerateWithTheSameModelWhileCloseIsPending() {
        // Regression: the stream holds Model's (fair) read lock for its whole
        // duration, a queued close() waits for it, and a hook generating with
        // the SAME model used to queue behind that writer — S waits for R waits
        // for C waits for S. The stream thread lends its read hold to the repair
        // worker so the hook's call takes no lock at all.
        assertTimeoutPreemptively(Duration.ofSeconds(30), () -> {
            try (MockProviderServer server = new MockProviderServer()) {
                server.setContentType("text/event-stream");
                server.setResponses(toolCallSse("{\"city\":\"Tokyo\"}"),
                                    textResponse("{\"location\":\"Tokyo\"}"));

                final Model raw =
                    Model.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl());
                final TypedModel model = TypedModel.of(raw);
                final CountDownLatch hookStarted = new CountDownLatch(1);
                final AtomicReference<Throwable> closerFailure = new AtomicReference<>();
                Thread closer = new Thread(() -> {
                    try {
                        hookStarted.await();
                        raw.close();
                    } catch (Throwable t) {
                        closerFailure.set(t);
                    }
                }, "closer");
                closer.start();

                Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
                    .tools(Collections.singletonList(strictWeatherTool()))
                    .repairToolCall(context -> {
                        hookStarted.countDown();
                        // Let the closer actually enqueue for the write lock;
                        // without it the race the test reproduces may not happen.
                        Thread.sleep(200);
                        // The mock server's content type is global, so flip it
                        // now that the SSE response has already been written.
                        server.setContentType("application/json");
                        return Types.RawToolCall.builder()
                            .toolCallId(context.getToolCall().getToolCallId())
                            .toolName(context.getToolCall().getToolName())
                            .input(model.generateText(
                                "fix these arguments: " + context.getToolCall().getInput())
                                .getText())
                            .build();
                    })
                    .build();

                List<Types.StreamPart> parts = new ArrayList<>();
                model.streamText("What is the weather in Tokyo?", options,
                    parts::add, () -> {}, error -> { throw new AssertionError(error); });

                closer.join(10_000);
                assertThat(closer.isAlive()).isFalse();
                assertThat(closerFailure.get()).isNull();

                List<Types.StreamPart.ToolCall> calls = parts.stream()
                    .filter(p -> p instanceof Types.StreamPart.ToolCall)
                    .map(p -> (Types.StreamPart.ToolCall) p)
                    .collect(Collectors.toList());
                assertThat(calls).hasSize(1);
                assertThat(calls.get(0).getInvalid()).isNull();
                assertThat(calls.get(0).getInput().get("location").asText()).isEqualTo("Tokyo");
                raw.close(); // idempotent; the closer already did it
            }
        });
    }

    @Test
    void aThrowingStreamHookReportsAToolCallRepairError() {
        try (MockProviderServer server = new MockProviderServer()) {
            server.setContentType("text/event-stream");
            server.setResponseBody(toolCallSse("{\"city\":\"Tokyo\"}"));
            Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
                .tools(Collections.singletonList(strictWeatherTool()))
                .repairToolCall(context -> {
                    throw new IllegalStateException("repair model unavailable");
                })
                .build();

            try (TypedModel model =
                     TypedModel.openaiWithBase("sk-test-fake-key", "gpt-4o", server.baseUrl())) {
                Types.StreamPart.ToolCall call =
                    model.streamTextStream("What is the weather in Tokyo?", options)
                        .filter(p -> p instanceof Types.StreamPart.ToolCall)
                        .map(p -> (Types.StreamPart.ToolCall) p)
                        .findFirst().orElseThrow(AssertionError::new);

                assertThat(call.getInvalid()).isTrue();
                assertThat(call.getError().get("ToolCallRepair").get("cause").get("Other").asText())
                    .isEqualTo("repair model unavailable");
            }
        }
    }

    /** A plain OpenAI text response whose content is {@code text}. */
    private static String textResponse(String text) {
        return new JSONObject()
            .put("id", "chatcmpl-repair")
            .put("model", "gpt-4o")
            .put("choices", new JSONArray().put(new JSONObject()
                .put("message", new JSONObject().put("role", "assistant").put("content", text))
                .put("finish_reason", "stop")))
            .put("usage", new JSONObject()
                .put("prompt_tokens", 4).put("completion_tokens", 4).put("total_tokens", 8))
            .toString();
    }

    /** {@code get_weather}, schema-strict so a {@code city} argument is rejected. */
    private static Types.Tool strictWeatherTool() {
        ObjectNode schema = Types.AimuxJson.MAPPER.createObjectNode();
        schema.put("type", "object");
        schema.set("properties", Types.AimuxJson.MAPPER.createObjectNode()
            .set("location", Types.AimuxJson.MAPPER.createObjectNode().put("type", "string")));
        schema.set("required", Types.AimuxJson.MAPPER.createArrayNode().add("location"));
        schema.put("additionalProperties", false);
        return Types.Tool.Function.builder().name("get_weather").inputSchema(schema).build();
    }

    private static String toolCallResponse(String arguments) {
        return new JSONObject()
            .put("id", "chatcmpl-tc")
            .put("model", "gpt-4o")
            .put("choices", new JSONArray().put(new JSONObject()
                .put("message", new JSONObject()
                    .put("role", "assistant")
                    .put("content", JSONObject.NULL)
                    .put("tool_calls", new JSONArray().put(new JSONObject()
                        .put("id", "call_abc")
                        .put("type", "function")
                        .put("function", new JSONObject()
                            .put("name", "get_weather")
                            .put("arguments", arguments)))))
                .put("finish_reason", "tool_calls")))
            .put("usage", new JSONObject()
                .put("prompt_tokens", 20).put("completion_tokens", 10).put("total_tokens", 30))
            .toString();
    }

    private static String toolCallSse(String arguments) {
        StringBuilder sb = new StringBuilder();
        sb.append("data: ").append(new JSONObject()
            .put("id", "1").put("model", "gpt-4o")
            .put("choices", new JSONArray().put(new JSONObject().put("delta", new JSONObject()
                .put("role", "assistant")
                .put("tool_calls", new JSONArray().put(new JSONObject()
                    .put("index", 0).put("id", "call_xyz").put("type", "function")
                    .put("function", new JSONObject().put("name", "get_weather")
                        .put("arguments", "")))))))).append("\n\n");
        sb.append("data: ").append(new JSONObject()
            .put("id", "1").put("model", "gpt-4o")
            .put("choices", new JSONArray().put(new JSONObject().put("delta", new JSONObject()
                .put("tool_calls", new JSONArray().put(new JSONObject()
                    .put("index", 0)
                    .put("function", new JSONObject().put("arguments", arguments)))))))).append("\n\n");
        sb.append("data: ").append(new JSONObject()
            .put("id", "1").put("model", "gpt-4o")
            .put("choices", new JSONArray().put(new JSONObject()
                .put("delta", new JSONObject()).put("finish_reason", "tool_calls")))
            .put("usage", new JSONObject()
                .put("prompt_tokens", 5).put("completion_tokens", 2).put("total_tokens", 7)))
            .append("\n\n");
        sb.append("data: [DONE]\n\n");
        return sb.toString();
    }

    // ── the hook never crosses the ABI ───────────────────────────────────────

    @Test
    void theHookIsNeverSerializedIntoTheOptions() throws Exception {
        Types.GenerateTextOptions options = Types.GenerateTextOptions.builder()
            .instructions("be terse")
            .repairToolCall(context -> null)
            .build();

        String json = Types.AimuxJson.MAPPER.writeValueAsString(options);
        assertThat(json).doesNotContain("repair");
        assertThat(options.getRepairToolCall()).isNotNull();
    }

    // ── fixtures ─────────────────────────────────────────────────────────────

    private static final String PROMPT = "\"weather in Singapore?\"";

    private static String opts(boolean withTools) {
        String tools = "\"tools\":[{\"type\":\"function\",\"name\":\"weather\",\"input_schema\":"
            + "{\"type\":\"object\",\"properties\":{\"city\":{\"type\":\"string\"}},"
            + "\"required\":[\"city\"],\"additionalProperties\":false}}],";
        return "{" + (withTools ? tools : "") + "\"instructions\":\"be terse\"}";
    }

    /** The shared fixture's invalid call: {@code weather({"town": …})} against a {@code city} schema. */
    private static String invalidCall() {
        return "{\"tool_call_id\":\"call-1\",\"tool_name\":\"weather\","
            + "\"input\":{\"town\":\"Singapore\"},\"dynamic\":true,\"invalid\":true,"
            + "\"error\":{\"InvalidToolInput\":{\"tool_name\":\"weather\","
            + "\"tool_input\":\"{\\\"town\\\":\\\"Singapore\\\"}\",\"cause\":\"schema mismatch\"}}}";
    }

    private static String result() {
        return "{\"text\":\"\",\"tool_calls\":[" + invalidCall() + "],"
            + "\"response_messages\":[{\"role\":\"assistant\",\"content\":["
            + "{\"type\":\"text\",\"text\":\"checking\"},"
            + "{\"type\":\"tool_call\",\"tool_call_id\":\"call-1\",\"tool_name\":\"weather\","
            + "\"input\":{\"town\":\"Singapore\"}}]}]}";
    }

    private static JsonNode json(String raw) throws IOException {
        return Types.AimuxJson.MAPPER.readTree(raw);
    }

    private static String text(JsonNode node) {
        return node == null || node.isNull() ? null : node.toString();
    }

    private static JsonNode loadFixture() throws IOException {
        for (String candidate : FIXTURE_CANDIDATES) {
            File f = new File(candidate);
            if (f.isFile()) {
                return Types.AimuxJson.MAPPER.readTree(f);
            }
        }
        throw new IllegalStateException(
            "cannot find tool-call-repair.json; tried " + Arrays.toString(FIXTURE_CANDIDATES)
                + " from " + new File(".").getAbsolutePath());
    }
}
