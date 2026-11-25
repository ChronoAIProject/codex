const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const { createHash } = require("node:crypto");
const { mkdtempSync, rmSync, writeFileSync } = require("node:fs");
const { createServer } = require("node:http");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { createInterface } = require("node:readline");
const { test } = require("node:test");
const { setTimeout: delay } = require("node:timers/promises");

const partial = "One partial model response.";
const bridge = process.env.CMA_ACP_TEST_BRIDGE ??
  "/usr/local/lib/node_modules/@agentclientprotocol/codex-acp/dist/index.js";
const binary = process.env.CMA_CODEX_TEST_BINARY ?? "/usr/local/bin/codex";

function dispatchKey(conversation, turn) {
  const bytes = Buffer.from(conversation);
  const integer = value => {
    const buffer = Buffer.alloc(8);
    buffer.writeBigUInt64BE(BigInt(value));
    return buffer;
  };
  return "cma-prompt-v1-" + createHash("sha256")
    .update("cma-native-prompt-v1\0")
    .update(integer(bytes.length))
    .update(bytes)
    .update(integer(1))
    .update(integer(turn))
    .digest("hex");
}

async function modelFixture() {
  const state = { dispatches: 0, error: null };
  const server = createServer(async (request, response) => {
    try {
      if (request.method === "GET" && request.url === "/v1/models") {
        response.writeHead(200, { "content-type": "application/json" });
        response.end(JSON.stringify({ data: [], models: [] }));
        return;
      }
      assert.equal(request.method, "POST");
      assert.equal(request.url, "/v1/responses");
      for await (const chunk of request) void chunk;
      const dispatch = ++state.dispatches;
      response.writeHead(200, { "content-type": "text/event-stream" });
      const send = event => response.write("data: " + JSON.stringify(event) + "\n\n");
      send({ type: "response.created", response: {
        id: "fixture-response-" + dispatch, status: "in_progress", output: [],
      } });
      send({ type: "response.output_item.added", output_index: 0, item: {
        id: "fixture-message-" + dispatch, type: "message", role: "assistant",
        status: "in_progress", content: [],
      } });
      send({ type: "response.content_part.added", output_index: 0, content_index: 0,
        part: { type: "output_text", text: "", annotations: [] } });
      // Deliver partial output asynchronously and leave the provider turn open.
      await delay(100);
      if (!response.destroyed) send({ type: "response.output_text.delta", output_index: 0,
        content_index: 0, item_id: "fixture-message-" + dispatch, delta: partial });
    } catch (error) {
      state.error = error;
      response.destroy();
    }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  return {
    state,
    origin: "http://127.0.0.1:" + server.address().port,
    async close() {
      server.closeAllConnections();
      await new Promise(resolve => server.close(resolve));
      if (state.error) throw state.error;
    },
  };
}

async function exercise({ sealed, enabled = sealed, capture, nativeBinary = binary }) {
  const directory = mkdtempSync(join(tmpdir(), "cma-stop-seal-"));
  const model = await modelFixture();
  let child, reader;
  try {
    writeFileSync(join(directory, "config.toml"), [
      'model="gpt-6"', 'model_provider="fixture"', 'web_search="disabled"',
      'approval_policy="never"', '[model_providers.fixture]',
      'name="Local fixture"', 'base_url="' + model.origin + '/v1"',
      'wire_api="responses"', 'requires_openai_auth=false',
    ].join("\n"));
    child = spawn(process.execPath, [bridge], {
      cwd: directory, detached: true, stdio: ["pipe", "pipe", "inherit"],
      env: {
        PATH: process.env.PATH, CODEX_HOME: directory, CODEX_PATH: nativeBinary,
        CHRONO_CODEX_EXACT_TURN_STOP_V1: enabled ? "1" : "0",
        CHRONO_CODEX_EXACT_TURN_SEAL_V1: enabled ? "1" : "0",
      },
    });
    reader = createInterface({ input: child.stdout });
    const frames = [];
    reader.on("line", raw => frames.push({ raw, value: JSON.parse(raw) }));
    const send = frame => child.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...frame }) + "\n");
    async function waitFor(predicate, start = 0) {
      const deadline = Date.now() + 12000;
      for (;;) {
        const index = frames.findIndex(({ value }, index) => index >= start && predicate(value));
        if (index >= 0) return index;
        assert.ok(child.exitCode === null && child.signalCode === null, "ACP exited unexpectedly");
        assert.ok(Date.now() < deadline, "Timed out waiting for the ACP fixture");
        await delay(25);
      }
    }
    async function call(id, method, params) {
      send({ id, method, params });
      const response = frames[await waitFor(value => value.id === id)].value;
      assert.equal(response.error, undefined);
      return response.result;
    }
    await call(1, "initialize", { protocolVersion: 1, clientCapabilities: {} });
    const { sessionId } = await call(2, "session/new", { cwd: directory, mcpServers: [] });
    const dispatch = dispatchKey(sessionId, 8);
    const prompt = [{ type: "text", text: "Reply once" }];
    send({ id: dispatch, method: "session/prompt", params: { sessionId, prompt } });
    const output = value => value.params?.update?.content?.text === partial;
    const first = await waitFor(output);

    const receipt = "d".repeat(32);
    send({ method: "cma/codex/stop-exact-turn", params: {
      sessionId, dispatchKey: dispatchKey(sessionId, 9), receipt: "e".repeat(32),
    } });
    send({ method: "cma/codex/stop-exact-turn", params: {
      sessionId, dispatchKey: dispatch, receipt,
    } });
    const markerMethod = "cma/codex/turn-stop-fence";
    if (sealed) {
      const fenceIndex = await waitFor(value => value.method === markerMethod);
      assert.ok(first < fenceIndex);
      const fence = frames[fenceIndex].value.params;
      assert.equal(fence.threadId, sessionId);
      assert.equal(fence.dispatchKey, dispatch);
      assert.equal(fence.dispatchSequence, 1);
      assert.equal(fence.receipt, receipt);
      assert.equal(frames.filter(({ value }) => value.id === dispatch).length, 0);
      if (capture) {
        const before = frames.slice(0, fenceIndex).map(({ raw }) => raw);
        writeFileSync(capture, JSON.stringify({
          conversation: sessionId, dispatch, before, marker: frames[fenceIndex].raw,
          last_payload_sha256: createHash("sha256").update(before.at(-1)).digest("hex"),
        }) + "\n", { mode: 0o600, flag: "wx" });
      }
      const successor = dispatchKey(sessionId, 10);
      send({ id: successor, method: "session/prompt", params: { sessionId, prompt } });
      await waitFor(output, fenceIndex + 1);
      send({ method: "session/cancel", params: { sessionId } });
      const result = frames[await waitFor(value => value.id === successor)].value;
      assert.equal(result.result?.stopReason, "cancelled");
      assert.equal(model.state.dispatches, 2);
      assert.equal(frames.filter(({ value }) => value.method === markerMethod).length, 1);
      assert.equal(frames.filter(({ value }) => value.id === dispatch).length, 0);
    } else {
      // A disabled feature or an unsupported producer preserves ordinary cancellation.
      send({ method: "session/cancel", params: { sessionId } });
      const result = frames[await waitFor(value => value.id === dispatch)].value;
      assert.equal(result.result?.stopReason, "cancelled");
      assert.equal(model.state.dispatches, 1);
      assert.equal(frames.filter(({ value }) => value.method === markerMethod).length, 0);
    }
  } finally {
    reader?.close();
    if (child && child.exitCode === null && child.signalCode === null) {
      const exited = new Promise(resolve => child.once("exit", resolve));
      process.kill(-child.pid, "SIGKILL");
      await exited;
    }
    await model.close();
    rmSync(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
}

test("packaged producer seals the exact dispatched turn and admits its successor",
  { timeout: 45000 }, () => exercise({ sealed: true, capture: process.env.CMA_NATIVE_STOP_TRACE }));
test("disabled administrative stop preserves ordinary cancellation",
  { timeout: 45000 }, () => exercise({ sealed: false }));
if (process.env.CMA_CODEX_TEST_LEGACY_BINARY) {
  test("unqualified producer with the feature enabled emits no seal",
    { timeout: 45000 }, () => exercise({
      sealed: false, enabled: true, nativeBinary: process.env.CMA_CODEX_TEST_LEGACY_BINARY,
    }));
}
