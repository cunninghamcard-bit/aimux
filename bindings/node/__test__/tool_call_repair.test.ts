// tool_call_repair.test.ts — host-side tool-call repair (RFC-0035).
//
// The wrapper calls generate/stream with no hook, spots `invalid` tool calls in
// the JSON it gets back, runs the user's `repairToolCall` in JavaScript, and
// hands the outcome to the pure native functions. Same mock-HTTP-server pattern
// as wrapper.test.ts — no real API calls. The last test replays the shared
// contract fixture against the three native functions directly.

import test from 'ava'
import { readFileSync } from 'node:fs'
import { createServer, type Server } from 'node:http'

import { openai, generateText, streamText } from '../src/index.ts'
import type { RawToolCall, StreamPart, Tool, ToolCallRepairContext } from '../src/index.ts'
import {
  applyToolCallRepair,
  applyToolCallRepairToResult,
  toolCallRepairContext,
} from '../src/native.ts'

// ── Mock server helpers (same shape as wrapper.test.ts) ────────────────────

function startMockServer(
  handler: (req: any, res: any) => void,
): Promise<{ server: Server; url: string }> {
  return new Promise((resolve) => {
    const server = createServer(handler)
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address() as any
      resolve({ server, url: `http://127.0.0.1:${addr.port}` })
    })
  })
}

function closeServer(server: Server): Promise<void> {
  return new Promise((resolve) => server.close(() => resolve()))
}

function respondWith(body: string, contentType = 'application/json') {
  return (_req: any, res: any) => {
    res.writeHead(200, { 'content-type': contentType })
    res.end(body)
  }
}

// ── Fixtures: one tool, and a model that gets its arguments wrong ──────────

const weatherTool: Tool = {
  type: 'function',
  name: 'weather',
  description: 'Weather for a city',
  input_schema: {
    type: 'object',
    properties: { city: { type: 'string' } },
    required: ['city'],
    additionalProperties: false,
  },
}

/** `{"town": …}` violates the schema, so core returns `invalid: true`. */
function toolCallResponse(args: string) {
  return JSON.stringify({
    id: 'chatcmpl-repair',
    model: 'gpt-4o',
    choices: [
      {
        message: {
          role: 'assistant',
          content: 'checking',
          tool_calls: [
            { id: 'call-1', type: 'function', function: { name: 'weather', arguments: args } },
          ],
        },
        finish_reason: 'tool_calls',
      },
    ],
    usage: { prompt_tokens: 5, completion_tokens: 2, total_tokens: 7 },
  })
}

const INVALID_TOOL_CALL = toolCallResponse('{"town":"Singapore"}')
const VALID_TOOL_CALL = toolCallResponse('{"city":"Singapore"}')

const TEXT_RESPONSE = JSON.stringify({
  id: 'chatcmpl-text',
  model: 'gpt-4o',
  choices: [{ message: { role: 'assistant', content: '{"city":"Singapore"}' }, finish_reason: 'stop' }],
  usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
})

const INVALID_TOOL_CALL_STREAM = [
  'data: {"id":"1","model":"gpt-4o","choices":[{"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"weather","arguments":""}}]}}]}\n\n',
  'data: {"id":"1","model":"gpt-4o","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\\"town\\":\\"Singapore\\"}"}}]}}]}\n\n',
  'data: {"id":"1","model":"gpt-4o","choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}\n\n',
  'data: [DONE]\n\n',
].join('')

/** A repair that rewrites `town` to `city`. */
const fixCity = (ctx: ToolCallRepairContext): RawToolCall => ({
  tool_call_id: ctx.tool_call.tool_call_id,
  tool_name: ctx.tool_call.tool_name,
  input: ctx.tool_call.input.replace('"town"', '"city"'),
})

// ── generateText ──────────────────────────────────────────────────────────

test('repair: a repaired call is valid and the response messages are patched', async (t) => {
  const { server, url } = await startMockServer(respondWith(INVALID_TOOL_CALL))
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const seen: ToolCallRepairContext[] = []
    const result = await generateText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      instructions: 'be terse',
      repairToolCall: (ctx) => {
        seen.push(ctx)
        return fixCity(ctx)
      },
    })

    // The context mirrors the AI SDK repair argument.
    t.is(seen.length, 1)
    t.is(seen[0].tool_call.input, '{"town":"Singapore"}', 'raw argument text, not parsed')
    t.truthy(seen[0].error)
    t.deepEqual(seen[0].messages, [{ role: 'user', content: 'weather in Singapore?' }])
    t.is(seen[0].instructions, 'be terse')
    t.is(seen[0].tools.length, 1)

    const call = result.tool_calls[0]
    t.deepEqual(call.input, { city: 'Singapore' })
    t.falsy(call.invalid)
    t.falsy(call.error)

    // The transcript must carry the repaired input too, or the next turn sends
    // the broken arguments back to the model.
    const assistant = result.response_messages[0] as any
    const part = assistant.content.find((c: any) => c.type === 'tool_call')
    t.deepEqual(part.input, { city: 'Singapore' })
  } finally {
    await closeServer(server)
  }
})

test('repair: returning null keeps the invalid call and its original error', async (t) => {
  const { server, url } = await startMockServer(respondWith(INVALID_TOOL_CALL))
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      repairToolCall: () => null,
    })

    const call = result.tool_calls[0]
    t.true(call.invalid === true)
    t.true(call.error !== null && 'InvalidToolInput' in (call.error as any))
  } finally {
    await closeServer(server)
  }
})

test('repair: a throwing repair function yields ToolCallRepair with its message', async (t) => {
  const { server, url } = await startMockServer(respondWith(INVALID_TOOL_CALL))
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      repairToolCall: () => {
        throw new Error('repair model unavailable')
      },
    })

    const error = result.tool_calls[0].error as any
    t.true(result.tool_calls[0].invalid === true)
    t.deepEqual(error.ToolCallRepair.cause, { Other: 'repair model unavailable' })
    t.truthy(error.ToolCallRepair.original_error.InvalidToolInput)
  } finally {
    await closeServer(server)
  }
})

test('repair: a replacement that is still invalid yields ToolCallRepair', async (t) => {
  const { server, url } = await startMockServer(respondWith(INVALID_TOOL_CALL))
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      repairToolCall: (ctx) => ({ ...fixCity(ctx), input: '{"city":42}' }),
    })

    const error = result.tool_calls[0].error as any
    t.true(result.tool_calls[0].invalid === true)
    t.truthy(error.ToolCallRepair.cause.InvalidToolInput)
  } finally {
    await closeServer(server)
  }
})

test('repair: a call made without a tool set is never repaired', async (t) => {
  const { server, url } = await startMockServer(respondWith(INVALID_TOOL_CALL))
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    let calls = 0
    const result = await generateText(model, 'weather in Singapore?', {
      repairToolCall: () => {
        calls += 1
        return null
      },
    })

    t.is(calls, 0, 'no tool set — the AI SDK never repairs such a call')
    t.true(result.tool_calls[0].invalid === true)
  } finally {
    await closeServer(server)
  }
})

test('repair: a valid call never reaches the repair function', async (t) => {
  const { server, url } = await startMockServer(respondWith(VALID_TOOL_CALL))
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    let calls = 0
    const result = await generateText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      repairToolCall: () => {
        calls += 1
        return null
      },
    })

    t.is(calls, 0)
    t.deepEqual(result.tool_calls[0].input, { city: 'Singapore' })
  } finally {
    await closeServer(server)
  }
})

test('repair: the repair function may call back into aimux', async (t) => {
  // The repair runs outside any native call, so asking another model to rewrite
  // the arguments is just an ordinary await.
  const broken = await startMockServer(respondWith(INVALID_TOOL_CALL))
  const fixer = await startMockServer(respondWith(TEXT_RESPONSE))
  try {
    const model = await openai('test-key', 'gpt-4o', broken.url)
    const helper = await openai('test-key', 'gpt-4o', fixer.url)
    const result = await generateText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      repairToolCall: async (ctx) => {
        const fixed = await generateText(helper, `fix these arguments: ${ctx.tool_call.input}`)
        return { ...ctx.tool_call, input: fixed.text }
      },
    })

    t.deepEqual(result.tool_calls[0].input, { city: 'Singapore' })
  } finally {
    await closeServer(broken.server)
    await closeServer(fixer.server)
  }
})

// ── streamText ────────────────────────────────────────────────────────────

test('repair: streamText replaces the ToolCall part and leaves deltas alone', async (t) => {
  const { server, url } = await startMockServer(
    respondWith(INVALID_TOOL_CALL_STREAM, 'text/event-stream'),
  )
  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const parts: StreamPart[] = []
    for await (const part of streamText(model, 'weather in Singapore?', {
      tools: [weatherTool],
      repairToolCall: fixCity,
    })) {
      parts.push(part)
    }

    const toolCall = parts.find(
      (p): p is Extract<StreamPart, { ToolCall: unknown }> => 'ToolCall' in p,
    )
    t.truthy(toolCall)
    t.deepEqual(toolCall!.ToolCall.input, { city: 'Singapore' })
    t.falsy(toolCall!.ToolCall.invalid)

    // Input deltas are the provider's own text, forwarded untouched.
    const deltas = parts
      .filter((p): p is Extract<StreamPart, { ToolInputDelta: unknown }> => 'ToolInputDelta' in p)
      .map((p) => p.ToolInputDelta.delta)
    t.is(deltas.join(''), '{"town":"Singapore"}')
  } finally {
    await closeServer(server)
  }
})

// ── Shared contract fixture ───────────────────────────────────────────────

test('repair: the shared contract fixture replays through the native functions', (t) => {
  const fixture = JSON.parse(
    readFileSync(new URL('../../../contract-tests/fixtures/tool-call-repair.json', import.meta.url), 'utf8'),
  ) as { cases: any[] }

  const run = (c: any): string => {
    const opts = c.input.opts === null ? undefined : JSON.stringify(c.input.opts)
    switch (c.function) {
      case 'tool_call_repair_context':
        return toolCallRepairContext(JSON.stringify(c.input.tool_call), JSON.stringify(c.input.prompt), opts)
      case 'apply_tool_call_repair':
        return applyToolCallRepair(JSON.stringify(c.input.tool_call), opts, JSON.stringify(c.input.reply))
      case 'apply_tool_call_repair_to_result':
        return applyToolCallRepairToResult(
          JSON.stringify(c.input.result),
          opts,
          c.input.tool_call_id,
          JSON.stringify(c.input.reply),
        )
      default:
        throw new Error(`unknown fixture function ${c.function}`)
    }
  }

  for (const c of fixture.cases) {
    if (c.expected_error) {
      const err = t.throws(() => run(c), undefined, c.name)
      t.is(err?.constructor.name, `${c.expected_error}Error`, c.name)
    } else {
      t.deepEqual(JSON.parse(run(c)), c.expected, c.name)
    }
  }
})
