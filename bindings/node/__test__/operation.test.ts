import test from 'ava'
import { createServer } from 'node:http'
import { openai, generateText, streamText, streamTextAsOpenai, TimeoutError, RequestAbortedError } from '../src/index.ts'
import type { GenerateTextOptions, ToolCallRepairContext } from '../src/index.ts'

const tools = [{ type: 'function', name: 'weather', input_schema: { type: 'object' } }]
async function fixture(t: any) {
  const server = createServer((req, res) => {
    let body = ''
    req.on('data', c => body += c)
    req.on('end', () => {
      const input = JSON.parse(body)
      const call = { id: 'c1', type: 'function', function: { name: 'weather', arguments: '{' } }
      if (input.stream) {
        res.writeHead(200, { 'content-type': 'text/event-stream' })
        res.end([
          { choices: [{ delta: { tool_calls: [{ index: 0, ...call }] } }] },
          { choices: [{ delta: {}, finish_reason: 'tool_calls' }] },
        ].map(x => `data: ${JSON.stringify({ id: 'completion', model: 'mock', ...x })}\n\n`).join('') + 'data: [DONE]\n\n')
      } else {
        res.writeHead(200, { 'content-type': 'application/json' })
        res.end(JSON.stringify({ id: 'completion', model: 'mock', usage: {prompt_tokens: 1, completion_tokens: 1, total_tokens: 2}, choices: [{ message: { role: 'assistant', tool_calls: [call] }, finish_reason: 'tool_calls' }] }))
      }
    })
  })
  await new Promise<void>(r => server.listen(0, '127.0.0.1', r))
  t.teardown(() => new Promise<void>(r => server.close(() => r())))
  return await openai('fake', 'mock', { baseUrl: `http://127.0.0.1:${(server.address() as any).port}`, maxRetries: 0 })
}
function opts(repairToolCall: GenerateTextOptions['repairToolCall']): GenerateTextOptions { return { tools: tools as any, repairToolCall } }
const fixed = (c: ToolCallRepairContext) => ({ ...c.tool_call, input: '{"city":"北京"}' })

test('host repair awaits locally, preserves context, and supports nested generation', async t => {
  const model = await fixture(t)
  let calls = 0
  const result = await generateText(model, 'hi', opts(async c => {
    calls++
    t.false(c.signal.aborted)
    t.deepEqual(c.input_schema, { type: 'object' })
    const nested = await generateText(model, 'nested', opts(fixed))
    t.deepEqual(nested.tool_calls[0].input, { city: '北京' })
    return fixed(c)
  }))
  t.is(calls, 1)
  t.deepEqual(result.tool_calls[0].input, { city: '北京' })
})

test('host exceptions and invalid returns are repaired-call failures', async t => {
  const model = await fixture(t)
  for (const hook of [() => { throw new Error('host exploded') }, () => 42, (c: ToolCallRepairContext) => ({ ...fixed(c), provider_executed: 'invalid' })]) {
    const result = await generateText(model, 'hi', opts(hook as any))
    t.true(result.tool_calls[0].invalid)
    t.truthy((result.tool_calls[0].error as any).ToolCallRepair)
  }
})

test('deadline ends a never-resolving repair and notifies its signal', async t => {
  const model = await fixture(t)
  let signal: AbortSignal | undefined
  const options = { ...opts(c => { signal = c.signal; return new Promise(() => {}) }), timeout: { total_ms: 200 } } as GenerateTextOptions
  await t.throwsAsync(generateText(model, 'hi', options), { instanceOf: TimeoutError })
  t.true(signal?.aborted)
})

test('abort while awaiting repair retains the typed abort error', async t => {
  const model = await fixture(t)
  const abort = new AbortController()
  await t.throwsAsync(generateText(model, 'hi', opts(() => {
    abort.abort()
    return new Promise(() => {})
  }), abort.signal), { instanceOf: RequestAbortedError })
})

test('OpenAI streaming emits repaired arguments once; stream return cleans up', async t => {
  const model = await fixture(t)
  let argumentsText = ''
  for await (const chunk of streamTextAsOpenai(model, 'hi', opts(fixed))) {
    for (const call of chunk.choices[0]?.delta.tool_calls ?? []) argumentsText += call.function?.arguments ?? ''
  }
  t.deepEqual(JSON.parse(argumentsText), { city: '北京' })
  for await (const part of streamText(model, 'hi', opts(fixed))) { t.truthy(part); break }
})

test('terminal observation cancels a hook while the stream consumer is paused', async t => {
  const model = await fixture(t)
  let observed!: () => void
  const cancelled = new Promise<void>(resolve => { observed = resolve })
  const stream = streamText(model, 'hi', { ...opts(c => {
    c.signal.addEventListener('abort', observed, { once: true })
    return new Promise(() => {})
  }), timeout: { total_ms: 200 } } as GenerateTextOptions)
  try {
    // StreamStart is available before the tool repair completes.
    t.false((await stream.next()).done)
    const outcome = await Promise.race([cancelled.then(() => 'cancelled'), new Promise(resolve => setTimeout(() => resolve('stuck'), 1000))])
    t.is(outcome, 'cancelled')
  } finally { await stream.return(undefined) }
})
