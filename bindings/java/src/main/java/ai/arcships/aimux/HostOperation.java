package ai.arcships.aimux;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.sun.jna.Pointer;
import com.sun.jna.ptr.IntByReference;
import com.sun.jna.ptr.PointerByReference;
import java.io.Closeable;
import java.io.IOException;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.concurrent.CancellationException;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Consumer;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/** Data-only C transport. No Java object is retained by Rust. */
final class HostOperation implements Closeable {
    private final AtomicLong handle;
    private final ToolCallRepairFunction repair;

    HostOperation(Model model, String mode, String prompt, String options, ToolCallRepairFunction repair) {
        ObjectNode request = Types.AimuxJson.MAPPER.createObjectNode();
        request.put("protocol_version", 1).put("mode", mode);
        request.set("prompt", parse(prompt));
        request.set("options", parse(options == null ? "{}" : options));
        request.put("repair_tool_call", true);
        this.repair = repair;
        handle = new AtomicLong(model.startOperation(request.toString()));
    }

    private static JsonNode parse(String text) {
        try { return Types.AimuxJson.MAPPER.readTree(text); }
        catch (IOException e) { throw new IllegalArgumentException("invalid operation JSON", e); }
    }

    /** ANY coordinator preserves the caller's thread for synchronous hooks. */
    String next() {
        while (handle.get() != 0) {
            if (Thread.currentThread().isInterrupted()) { close(); throw new CancellationException("operation interrupted"); }
            long h = handle.get();
            PointerByReference out = new PointerByReference();
            IntByReference state = new IntByReference();
            Pointer error = AimuxFFI.INSTANCE.aimux_operation_next(h, 2, 100, out, state);
            if (error != null) {
                RuntimeException failure = AimuxResult.expectAimuxError(error, "operation_next");
                if (handle.get() == 0) return null;
                throw failure;
            }
            if (state.getValue() == 1) continue;
            if (state.getValue() == 2) return null;
            if (state.getValue() != 0) throw new IllegalStateException("concurrent operation reader");
            String wire;
            try { wire = out.getValue().getString(0, "UTF-8"); }
            finally { AimuxFFI.INSTANCE.aimux_free_string(out.getValue()); }
            JsonNode event = parse(wire);
            String type = event.path("type").asText();
            if (!type.equals("repair_request")) return event.get(type.equals("part") ? "part" : "result").toString();
            ObjectNode reply = Types.AimuxJson.MAPPER.createObjectNode();
            try {
                Types.ToolCallRepairContext context = Types.AimuxJson.MAPPER.treeToValue(event.get("context"), Types.ToolCallRepairContext.class);
                Types.RawToolCall value = repair.repair(context);
                if (value == null) reply.put("type", "unchanged");
                else {
                    if (value.getToolCallId() == null || value.getToolName() == null || value.getInput() == null) throw new IllegalArgumentException("repair returned null required fields");
                    reply.put("type", "repaired").set("tool_call", Types.AimuxJson.MAPPER.valueToTree(value));
                }
            } catch (Exception e) { reply.removeAll(); reply.put("type", "failed").put("message", e.toString()); }
            if (handle.get() == 0) return null;
            IntByReference status = new IntByReference();
            error = AimuxFFI.INSTANCE.aimux_operation_reply(h, event.path("request_id").asText(), reply.toString(), status);
            if (error != null) throw AimuxResult.expectAimuxError(error, "operation_reply");
            if (status.getValue() != 0 && status.getValue() != 3) throw new IllegalStateException("unexpected operation reply status");
        }
        return null;
    }

    @Override public void close() {
        long h = handle.getAndSet(0);
        if (h != 0) AimuxFFI.INSTANCE.aimux_operation_drop(h);
    }

    static String result(Model model, String mode, String prompt, String options, ToolCallRepairFunction repair) {
        try (HostOperation op = new HostOperation(model, mode, prompt, options, repair)) {
            String value = op.next();
            if (value == null) throw new IllegalStateException("operation ended without a result");
            return value;
        }
    }

    /** Callers must close a short-circuited stream (try-with-resources). */
    static Stream<String> stream(Model model, String mode, String prompt, String options, ToolCallRepairFunction repair) {
        HostOperation op = new HostOperation(model, mode, prompt, options, repair);
        return StreamSupport.stream(new Spliterators.AbstractSpliterator<String>(Long.MAX_VALUE, Spliterator.ORDERED) {
            @Override public boolean tryAdvance(Consumer<? super String> consumer) {
                try {
                    String value = op.next();
                    if (value == null) { op.close(); return false; }
                    consumer.accept(value); return true;
                } catch (RuntimeException | Error e) { op.close(); throw e; }
            }
        }, false).onClose(op::close);
    }
}
