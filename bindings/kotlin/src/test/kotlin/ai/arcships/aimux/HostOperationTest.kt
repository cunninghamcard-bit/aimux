package ai.arcships.aimux

import java.io.File
import kotlinx.serialization.json.*
import kotlinx.serialization.encodeToString
import org.junit.jupiter.api.Test
import org.assertj.core.api.Assertions.assertThat

class HostOperationTest {
    @Test fun `repair stays on caller thread and supports nested generation`() {
        val fixture = AimuxJson.parseToJsonElement(File("../../contract-tests/host-operation.json").readText()).jsonObject
        val server = MockProviderServer()
        server.responseBody = fixture.getValue("response").toString()
        try {
            TypedModel.openai("fake", "mock", server.baseUrl).use { model ->
                val caller = Thread.currentThread()
                val data = AimuxJson.decodeFromJsonElement(GenerateTextOptions.serializer(), fixture.getValue("options"))
                val inner = data.copy(repairToolCall = { it.toolCall.copy(input = "{}") })
                val outer = data.copy(repairToolCall = {
                    assertThat(Thread.currentThread()).isSameAs(caller)
                    assertThat(model.generateText("nested", inner).toolCalls.first().input).isEqualTo(JsonObject(emptyMap()))
                    it.toolCall.copy(input = "{\"city\":\"北京\"}")
                })
                assertThat(AimuxJson.encodeToString(outer)).doesNotContain("repair")
                assertThat(model.generateText("hi", outer).toolCalls.first().input.jsonObject["city"]?.jsonPrimitive?.content).isEqualTo("北京")
            }
        } finally { server.stop() }
    }
}
