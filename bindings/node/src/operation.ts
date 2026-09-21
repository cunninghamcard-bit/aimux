// Shared operation driver. No JS function is passed into the native addon.
import type { Model } from './native.ts'
import type { GenerateTextOptions as GenerationData, ModelMessage, Tool, AiMuxError, JsonValue } from './types'

export type RawToolCall = {
  tool_call_id: string; tool_name: string; input: string
  provider_executed?: boolean | null; dynamic?: boolean | null
  thought_signature?: string | null; provider_metadata?: JsonValue | null
}
export type ToolCallRepairContext = {
  tool_call: RawToolCall; error: AiMuxError; input_schema: JsonValue
  tools: Tool[]; messages: ModelMessage[]; instructions?: string
  /** Local notification; never serialized into the operation protocol. */
  signal: AbortSignal
}
export type RepairToolCall = (context: ToolCallRepairContext) => RawToolCall | null | Promise<RawToolCall | null>
export type GenerateTextOptions = GenerationData & { repairToolCall?: RepairToolCall }
type Mode = 'generate_text' | 'generate_object' | 'consume_stream_text' | 'stream_text' | 'generate_text_as_openai' | 'stream_text_as_openai'
type Event = { type: string; request_id?: string; context?: Omit<ToolCallRepairContext, 'signal'>; part?: unknown; result?: unknown }

export async function* operationEvents(model: Model, mode: Mode, prompt: string | ModelMessage[], options: GenerateTextOptions, signal?: AbortSignal): AsyncGenerator<Event> {
  const { repairToolCall, ...data } = options
  if (typeof repairToolCall !== 'function') throw new TypeError('repairToolCall must be a function')
  const op = await model.startOperation({ protocol_version: 1, mode, prompt, options: data, repair_tool_call: true })
  const local = new AbortController()
  const stop = Symbol('operation ended')
  const stopped = new Promise<typeof stop>(resolve => local.signal.addEventListener('abort', () => resolve(stop), { once: true }))
  const cancel = () => op.cancel()
  signal?.addEventListener('abort', cancel, { once: true })
  if (signal?.aborted) cancel()
  const terminal = op.finished().then(() => local.abort())
  const control = (async () => {
    while (!local.signal.aborted) {
      const event = await op.next(0) as Event | null
      if (!event) return
      if (event.type !== 'repair_request' || !event.request_id || !event.context) throw new Error('invalid host request')
      let status: number
      try {
        const result = await Promise.race([
          Promise.resolve().then(() => repairToolCall({ ...event.context!, signal: local.signal })), stopped,
        ])
        if (result === stop) return
        // Decode once against the shared Rust schema, including optional fields.
        status = op.reply(event.request_id, result === null ? { type: 'unchanged' } : { type: 'repaired', tool_call: result })
      } catch (error) {
        if (local.signal.aborted) return
        status = op.reply(event.request_id, { type: 'failed', message: String(error) })
      }
      if (status === 3) return
      if (status !== 0) throw new Error(`unexpected repair reply status: ${status}`)
    }
  })()
  // A driver failure must cancel generation and never become an unhandled rejection.
  let driverError: unknown
  const controlled = control.catch(error => { driverError = error; op.cancel() })
  try {
    while (true) {
      const event = await op.next(1) as Event | null
      if (!event) break
      yield event
    }
    if (driverError) throw driverError
  } finally {
    signal?.removeEventListener('abort', cancel)
    local.abort()
    await op.close()
    await Promise.all([controlled, terminal])
  }
}

export async function operationResult(model: Model, mode: Mode, prompt: string | ModelMessage[], options: GenerateTextOptions, signal?: AbortSignal): Promise<unknown> {
  for await (const event of operationEvents(model, mode, prompt, options, signal)) {
    if (event.type === 'result') return event.result
  }
  throw new Error('operation ended without a result')
}
