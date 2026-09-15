package ai.arcships.aimux;

/**
 * One attempt to fix an invalid tool call (AI SDK {@code repairToolCall}).
 *
 * <p>Return the repaired call, or {@code null} to keep the original validation
 * error. Core parses and validates the returned call from scratch.
 *
 * <p>It runs synchronously on the thread that called
 * {@code generateText} / {@code streamText}, while that call is in progress.
 * It must not call back into aimux: the FFI layer rejects that as a re-entrant
 * call ({@code AIMUX_E_FFI_REENTRANT_CALL}). Anything thrown here is caught by
 * {@link ToolCallRepair} (never unwinds into Rust), leaves the original error
 * on the tool call, and is kept as {@link ToolCallRepair#lastError()}.
 */
public interface ToolCallRepairFunction {

    Types.RawToolCall repair(Types.ToolCallRepairContext context) throws Exception;
}
