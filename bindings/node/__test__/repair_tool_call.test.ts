// repair_tool_call.test.ts — the AI SDK `repairToolCall` hook, JS side.
//
// The mock server returns a tool call whose arguments are truncated
// (`{"location":"Tokyo"` — missing brace), so the core's JSON parse fails and
// the hook gets its one attempt. Same mock-HTTP-server pattern as e2e.test.ts.

import test from 'ava'
import { createServer, type Server } from 'node:http'

import { openai, generateText } from '../src/index.ts'
import type { Tool, ToolCallRepairContext } from '../src/index.ts'

const BROKEN_TOOL_CALL_RESPONSE = JSON.stringify({
  id: 'chatcmpl-repair',
  model: 'gpt-4o',
  choices: [{
    message: {
      role: 'assistant',
      content: null,
      tool_calls: [{
        id: 'call_repair',
        type: 'function',
        function: { name: 'get_weather', arguments: '{"location":"Tokyo"' },
      }],
    },
    finish_reason: 'tool_calls',
  }],
  usage: { prompt_tokens: 20, completion_tokens: 10, total_tokens: 30 },
})

const weatherTool: Tool = {
  type: 'function',
  name: 'get_weather',
  description: 'Get weather for a location',
  input_schema: {
    type: 'object',
    properties: { location: { type: 'string' } },
    required: ['location'],
  },
}

function startMockServer(): Promise<{ server: Server; url: string }> {
  return new Promise((resolve) => {
    const server = createServer((_req, res) => {
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(BROKEN_TOOL_CALL_RESPONSE)
    })
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address() as any
      resolve({ server, url: `http://127.0.0.1:${addr.port}` })
    })
  })
}

function closeServer(server: Server): Promise<void> {
  return new Promise((resolve) => server.close(() => resolve()))
}

test('repairToolCall: repaired arguments are parsed and validated from scratch', async (t) => {
  const { server, url } = await startMockServer()
  let seen: ToolCallRepairContext | null = null

  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, "What's the weather in Tokyo?", {
      tools: [weatherTool],
      repairToolCall: async (context) => {
        seen = context
        return { ...context.tool_call, input: `${context.tool_call.input}}` }
      },
    })

    t.is(result.tool_calls.length, 1)
    t.deepEqual(result.tool_calls[0].input, { location: 'Tokyo' })
    t.falsy(result.tool_calls[0].invalid)
    t.falsy(result.tool_calls[0].error)

    // The context carries the AI SDK repair arguments.
    const context = seen as unknown as ToolCallRepairContext
    t.is(context.tool_call.tool_name, 'get_weather')
    t.is(context.tool_call.input, '{"location":"Tokyo"')
    t.deepEqual(context.input_schema, weatherTool.input_schema)
    t.is(context.tools.length, 1)
    t.true(Array.isArray(context.messages))
    t.truthy(context.error)
  } finally {
    await closeServer(server)
  }
})

// The hook runs on the JS event loop while the native call waits on a tokio
// worker, so it may call aimux again — the documented Node guarantee, which the
// C ABI's re-entrancy guard does not give.
test('repairToolCall: the hook may call aimux again', async (t) => {
  let calls = 0
  const server = createServer((_req, res) => {
    calls += 1
    res.writeHead(200, { 'content-type': 'application/json' })
    // Second request = the one the hook makes; answer it with the fixed text.
    res.end(calls === 1 ? BROKEN_TOOL_CALL_RESPONSE : JSON.stringify({
      id: 'chatcmpl-fix',
      model: 'gpt-4o',
      choices: [{ message: { role: 'assistant', content: '{"location":"Tokyo"}' }, finish_reason: 'stop' }],
      usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
    }))
  })
  const url = await new Promise<string>((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address() as any
      resolve(`http://127.0.0.1:${addr.port}`)
    })
  })

  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, "What's the weather in Tokyo?", {
      tools: [weatherTool],
      repairToolCall: async (context) => {
        const fixed = await generateText(model, 'Fix these tool arguments.')
        return { ...context.tool_call, input: fixed.text }
      },
    })

    t.is(calls, 2, 'the hook reached the provider from inside the outer call')
    t.deepEqual(result.tool_calls[0].input, { location: 'Tokyo' })
    t.falsy(result.tool_calls[0].invalid)
  } finally {
    await closeServer(server)
  }
})

test('repairToolCall: returning null keeps the original error and raw text', async (t) => {
  const { server, url } = await startMockServer()

  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, "What's the weather in Tokyo?", {
      tools: [weatherTool],
      repairToolCall: () => null,
    })

    t.is(result.tool_calls.length, 1)
    t.true(result.tool_calls[0].invalid === true)
    t.is(result.tool_calls[0].input, '{"location":"Tokyo"')
    // The original validation failure, not a ToolCallRepair wrapper.
    t.truthy(result.tool_calls[0].error)
    t.false('ToolCallRepair' in (result.tool_calls[0].error as object))
  } finally {
    await closeServer(server)
  }
})

test('repairToolCall: a throwing hook yields a ToolCallRepair error', async (t) => {
  const { server, url } = await startMockServer()

  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, "What's the weather in Tokyo?", {
      tools: [weatherTool],
      repairToolCall: () => {
        throw new Error('repair exploded')
      },
    })

    t.is(result.tool_calls.length, 1)
    t.true(result.tool_calls[0].invalid === true)
    const error = result.tool_calls[0].error as any
    t.truthy(error.ToolCallRepair, 'error is the ToolCallRepair variant')
    t.truthy(error.ToolCallRepair.original_error)
    t.regex(JSON.stringify(error.ToolCallRepair.cause), /repair exploded/)
  } finally {
    await closeServer(server)
  }
})

test('repairToolCall: a reply that is neither a call nor an error is a ToolCallRepair error', async (t) => {
  const { server, url } = await startMockServer()

  try {
    const model = await openai('test-key', 'gpt-4o', url)
    const result = await generateText(model, "What's the weather in Tokyo?", {
      tools: [weatherTool],
      repairToolCall: () => ({}) as any,
    })

    t.true(result.tool_calls[0].invalid === true)
    const error = result.tool_calls[0].error as any
    t.truthy(error.ToolCallRepair)
    t.regex(JSON.stringify(error.ToolCallRepair.cause), /neither a RawToolCall nor/)
  } finally {
    await closeServer(server)
  }
})
