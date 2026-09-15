package ai.arcships.aimux;

import com.fasterxml.jackson.annotation.JsonValue;
import com.sun.jna.Pointer;

import java.util.Map;
import java.util.Objects;
import java.util.concurrent.ConcurrentHashMap;

/**
 * A registered {@link ToolCallRepairFunction}. Set it on
 * {@link Types.GenerateTextOptions.Builder#repairToolCall}; it serializes as
 * its FFI handle ({@code "repair_tool_call": <handle>} in {@code opts_json}),
 * which is how the function reaches Core.
 *
 * <p>Implements {@link AutoCloseable}: {@link #close()} releases the handle.
 * Until then the object stays reachable from a process-wide registry, so it
 * must be closed; a closed repair serializes as {@code null} (no repair).
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

    // Rust holds only a raw function pointer, and JNA tracks callbacks weakly:
    // an options object is dead the moment it is serialized, so without this a
    // GC during the HTTP wait frees the trampoline Rust is about to call.
    // Leaking a forgotten repair beats crashing on one.
    private static final Map<Long, ToolCallRepair> REGISTERED = new ConcurrentHashMap<>();

    private final ToolCallRepairFunction fn;

    private final AimuxFFI.ToolCallRepairCallback callback =
        new AimuxFFI.ToolCallRepairCallback() {
            @Override
            public Pointer invoke(Pointer contextJson, Pointer userData) {
                return ToolCallRepair.this.invoke(contextJson);
            }
        };

    private long handle;

    /** Register {@code fn} with the FFI layer. */
    public ToolCallRepair(ToolCallRepairFunction fn) {
        this.fn = Objects.requireNonNull(fn, "fn");
        this.handle = AimuxFFI.INSTANCE.aimux_tool_call_repair_new(callback, null);
        if (handle != 0L) {
            REGISTERED.put(handle, this);
        }
    }

    /**
     * Release the FFI handle and stop keeping this object alive. Idempotent.
     *
     * <p>Calls already in flight are disarmed and behave as if the function had
     * returned {@code null}, so closing is safe as long as no invocation is
     * executing on another thread at that instant.
     */
    @Override
    public synchronized void close() {
        if (handle != 0L) {
            AimuxFFI.INSTANCE.aimux_tool_call_repair_drop(handle);
            REGISTERED.remove(handle);
            handle = 0L;
        }
    }

    /** The FFI handle, which is how the function reaches {@code opts_json}. */
    @JsonValue
    synchronized Long handle() {
        return handle == 0L ? null : handle;
    }

    // Nothing may unwind into Rust: every failure comes back as the error
    // envelope, which Core records as ToolCallRepair {original_error, cause}.
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
            return AimuxFFI.INSTANCE.aimux_string_new(
                Types.AimuxJson.MAPPER.createObjectNode().put("error", t.toString()).toString());
        }
    }
}
