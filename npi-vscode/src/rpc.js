'use strict';
const { spawn } = require('node:child_process');
const { EventEmitter } = require('node:events');
const { createInterface } = require('node:readline');
const { TextDecoder } = require('node:util');

// NPi JSONL transport v2. Requests are correlated independently of event order.
class RpcClient extends EventEmitter {
  constructor(executable, args, options = {}) {
    super();
    this.pending = new Map();
    this.sequence = 0;
    this.chunk = null;
    this.stopping = false;
    this.maxBytes = 67108864;
    this.child = spawn(executable, args, { ...options, stdio: ['pipe', 'pipe', 'pipe'], shell: false });
    this.closed = new Promise(resolve => this.child.once('close', resolve));
    this.ready = new Promise((resolve, reject) => { this.resolveReady = resolve; this.rejectReady = reject; });
    this.ready.catch(() => {});
    this.lines = createInterface({ input: this.child.stdout, crlfDelay: Infinity });
    this.lines.on('line', line => {
      try { this.decode(JSON.parse(line)); }
      catch (error) { this.fail(error); this.child.kill(); }
    });
    this.child.stderr.setEncoding('utf8');
    this.child.stderr.on('data', text => this.emit('stderr', text));
    this.child.stdin.on('error', error => this.fail(error));
    this.child.on('error', error => this.fail(error));
    this.child.on('close', (code, signal) => {
      clearTimeout(this.killTimer);
      const error = new Error('NPi exited (' + (signal || code) + '). Open NPi: Show Agent Logs for details.');
      this.fail(error);
      this.emit('close', { code, signal, expected: this.stopping });
      this.lines.close();
    });
  }

  decode(frame) {
    if (frame.type === 'rpc_chunk') {
      if (!this.chunk) {
        if (frame.index !== 0 || !Number.isInteger(frame.count) || frame.count < 1 ||
            !Number.isInteger(frame.byteLength) || frame.byteLength < 1 || frame.byteLength > this.maxBytes ||
            typeof frame.chunkId !== 'string') throw new Error('Invalid NPi chunk header');
        this.chunk = { id: frame.chunkId, count: frame.count, bytes: frame.byteLength, parts: [], size: 0 };
      }
      const chunk = this.chunk;
      if (frame.chunkId !== chunk.id || frame.index !== chunk.parts.length || frame.count !== chunk.count ||
          frame.byteLength !== chunk.bytes || typeof frame.data !== 'string') throw new Error('Interrupted NPi chunk sequence');
      const bytes = Buffer.from(frame.data, 'base64');
      chunk.parts.push(bytes);
      chunk.size += bytes.length;
      if (chunk.size > chunk.bytes) throw new Error('NPi chunk exceeds declared length');
      if (chunk.parts.length === chunk.count) {
        if (chunk.size !== chunk.bytes) throw new Error('Incomplete NPi chunk payload');
        this.chunk = null;
        this.dispatch(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunk.parts))));
      }
      return;
    }
    if (this.chunk) throw new Error('Interrupted NPi chunk sequence');
    this.dispatch(frame);
  }

  dispatch(frame) {
    if (frame.type === 'ready') {
      this.maxBytes = frame.maxReassembledFrameBytes || this.maxBytes;
      (async () => {
        if (frame.supportedProtocolVersions?.includes(2)) await this.request('negotiate_protocol', { protocolVersion: 2 });
        this.resolveReady();
      })().catch(error => this.fail(error));
    } else if (frame.type === 'response') {
      const pending = this.pending.get(frame.id);
      if (pending) {
        this.pending.delete(frame.id);
        frame.success ? pending.resolve(frame.data) : pending.reject(new Error(frame.error));
      } else if (!frame.success) this.emit('fault', new Error(frame.error));
    }
    this.emit('frame', frame);
  }

  send(frame) {
    if (this.stopping || this.child.exitCode !== null || !this.child.stdin.writable) throw new Error('NPi is disconnected');
    const line = JSON.stringify(frame) + '\n';
    if (Buffer.byteLength(line) > 1048576) throw new Error('Message exceeds NPi’s 1 MiB request limit. Attach a smaller selection.');
    this.child.stdin.write(line);
  }

  request(type, fields = {}) {
    const id = 'vscode-' + ++this.sequence;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      try { this.send({ ...fields, type, id }); }
      catch (error) { this.pending.delete(id); reject(error); }
    });
  }

  fail(error) {
    this.rejectReady(error);
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    if (!this.stopping) this.emit('fault', error);
  }

  dispose() {
    if (this.stopping) return;
    // Abort active work and cancel dialogs before EOF; never kill another session.
    try { this.send({ type: 'abort' }); } catch {}
    this.stopping = true;
    this.child.stdin.end();
    this.killTimer = setTimeout(() => this.child.kill('SIGTERM'), 3000);
    this.killTimer.unref();
  }
}
module.exports = { RpcClient };
