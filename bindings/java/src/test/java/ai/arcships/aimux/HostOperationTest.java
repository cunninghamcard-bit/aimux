package ai.arcships.aimux;

import com.fasterxml.jackson.databind.JsonNode;
import java.nio.file.Paths;
import java.util.Collections;
import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class HostOperationTest {
    private JsonNode fixture() throws Exception {
        return Types.AimuxJson.MAPPER.readTree(Paths.get("../../contract-tests/host-operation.json").toFile());
    }
    private Types.GenerateTextOptions options(JsonNode fixture, ToolCallRepairFunction repair) throws Exception {
        Types.Tool tool = Types.AimuxJson.MAPPER.treeToValue(fixture.get("options").get("tools").get(0), Types.Tool.class);
        return Types.GenerateTextOptions.builder().tools(Collections.singletonList(tool)).repairToolCall(repair).build();
    }

    @Test void repairRunsOnCallerAndCanNest() throws Exception {
        JsonNode fixture = fixture();
        try (MockProviderServer server = new MockProviderServer()) {
            server.setResponseBody(fixture.get("response").toString());
            try (TypedModel model = TypedModel.openaiWithBase("fake", "mock", server.baseUrl())) {
                Thread caller = Thread.currentThread();
                Types.GenerateTextOptions inner = options(fixture, c -> c.getToolCall().withInput("{}"));
                Types.GenerateTextOptions outer = options(fixture, c -> {
                    assertThat(Thread.currentThread()).isSameAs(caller);
                    assertThat(model.generateText("nested", inner).getToolCalls().get(0).getInput().isObject()).isTrue();
                    return c.getToolCall().withInput("{\"city\":\"北京\"}");
                });
                assertThat(Types.AimuxJson.MAPPER.writeValueAsString(outer)).doesNotContain("repair");
                assertThat(model.generateText("hi", outer).getToolCalls().get(0).getInput().get("city").asText()).isEqualTo("北京");
            }
        }
    }

    @Test void closingModelInsideRepairDoesNotDeadlockOrInvalidateOperation() throws Exception {
        JsonNode fixture = fixture();
        try (MockProviderServer server = new MockProviderServer()) {
            server.setResponseBody(fixture.get("response").toString());
            try (Model raw = Model.openaiWithBase("fake", "mock", server.baseUrl())) {
                Types.GenerateTextOptions options = options(fixture, c -> { raw.close(); return c.getToolCall().withInput("{}"); });
                assertThat(TypedModel.of(raw).generateText("hi", options).getToolCalls().get(0).getInput().isObject()).isTrue();
            }
        }
    }
}
