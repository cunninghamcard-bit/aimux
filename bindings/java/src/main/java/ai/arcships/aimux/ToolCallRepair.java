package ai.arcships.aimux;

import com.fasterxml.jackson.annotation.JsonValue;
import com.sun.jna.Pointer;

import java.util.Objects;

/**
 * A registered {@link ToolCallRepairFunction}. Set it on
 * {@link Types.GenerateTextOptions.Builder#repairToolCall}; it serializes as
 * its FFI handle ({@code "repair_tool_call": <handle>} in {@code opts_json}),
 * which is how the function reaches Core.
 *
 * <p>Implements {@link AutoCloseable}: {@link #close()} releases the handle.
 * Close it once no call that references it is in flight; a closed repair
 * serializes as {@code null} (no repair).
 *
 * <pre>{@code
 * try (ToolCallRepair repair = new ToolCallRepair(ctx ->
 *          ctx.getToolCall().withInput(ctx.getToolCall().getInput() + "}"))) {
 *     Types.GenerateTextOptions opts = Types.GenerateTextOptions.builder()
 *         .tools(tools).repairToolCall(repair).build();
 *     model.generateText("...", opts);
 * }
 * }</pre>
 */
public final class ToolCallRepair implements AutoCloseable {

    private final ToolCallRepairFunction fn;

    // The native side keeps only a raw function pointer, so the JNA callback
    // must stay strongly referenced for as long as the handle is registered.
    private final AimuxFFI.ToolCallRepairCallback callback =
        new AimuxFFI.ToolCallRepairCallback() {
            @Override
            public Pointer invoke(Pointer contextJson, Pointer userData) {
                return ToolCallRepair.this.invoke(contextJson);
            }
        };

    private volatile Throwable lastError;
    private long handle;

    /** Register {@code fn} with the FFI layer. */
    public ToolCallRepair(ToolCallRepairFunction fn) {
        this.fn = Objects.requireNonNull(fn, "fn");
        this.handle = AimuxFFI.INSTANCE.aimux_tool_call_repair_new(callback, null);
    }

    /**
     * The last Throwable the repair function raised (or a failure to decode the
     * context / encode the repaired call). The C contract carries only
     * "repaired" or "not repaired", so a failing function leaves the original
     * validation error on the tool call; this is where the cause is kept.
     */
    public Throwable lastError() {
        return lastError;
    }

    /** Release the FFI handle. Idempotent; calls already in flight keep their clone. */
    @Override
    public synchronized void close() {
        if (handle != 0L) {
            AimuxFFI.INSTANCE.aimux_tool_call_repair_drop(handle);
            handle = 0L;
        }
    }

    /** The FFI handle, which is how the function reaches {@code opts_json}. */
    @JsonValue
    synchronized Long handle() {
        return handle == 0L ? null : handle;
    }

    // Nothing may unwind into Rust: every failure becomes NULL ("not repaired").
    private Pointer invoke(Pointer contextJson) {
        try {
            Types.ToolCallRepairContext context = Types.AimuxJson.MAPPER.readValue(
                contextJson.getString(0, "UTF-8"), Types.ToolCallRepairContext.class);
            Types.RawToolCall repaired = fn.repair(context);
            if (repaired == null) {
                return null;
            }
            return AimuxFFI.INSTANCE.aimux_string_new(
                Types.AimuxJson.MAPPER.writeValueAsString(repaired));
        } catch (Throwable t) {
            lastError = t;
            return null;
        }
    }
}
