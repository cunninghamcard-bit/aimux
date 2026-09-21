package ai.arcships.aimux;

/** Runs in the Java caller/iterator thread, after the native receive returns.
 * Return null to retain the invalid call. Exceptions become repair failures. */
@FunctionalInterface
public interface ToolCallRepairFunction {
    Types.RawToolCall repair(Types.ToolCallRepairContext context) throws Exception;
}
