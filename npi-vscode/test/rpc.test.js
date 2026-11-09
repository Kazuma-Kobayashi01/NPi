'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { once } = require('node:events');
const { RpcClient } = require('../src/rpc');

function launch(behavior) {
  return new RpcClient(process.execPath, ['-e',
    'const readline = require("node:readline");'
    + 'const send = value => process.stdout.write(JSON.stringify(value)+"\\n");'
    + 'send({type:"ready",supportedProtocolVersions:[1,2],maxReassembledFrameBytes:67108864});'
    + 'readline.createInterface({input:process.stdin}).on("line",line=>{const request=JSON.parse(line);'
    + 'if(request.type==="negotiate_protocol"){send({type:"response",id:request.id,success:true});return;}'
    + 'if(request.type==="abort")return;'
    + behavior + '});']);
}

test('large UTF-8 histories reassemble and concurrent responses retain their request identity', { timeout: 5000 }, async t => {
  const client = launch(
    'if(request.type==="get_state"){setTimeout(()=>send({type:"response",id:request.id,success:true,data:{sessionId:"session-A",isSettled:true}}),40);return;}'
    + 'const payload=Buffer.from(JSON.stringify({type:"response",id:request.id,success:true,data:{messages:[{role:"assistant",content:[{type:"text",text:"代码🙂".repeat(180000)}]}]}}));'
    + 'const split=37;const parts=[payload.subarray(0,split),payload.subarray(split)];'
    + 'parts.forEach((part,index)=>send({type:"rpc_chunk",chunkId:"history",index,count:2,byteLength:payload.length,data:part.toString("base64")}));');
  t.after(() => client.dispose());
  await client.ready;
  const state = client.request('get_state');
  const history = client.request('get_messages');
  assert.equal((await history).messages[0].content[0].text, '代码🙂'.repeat(180000));
  assert.equal((await state).sessionId, 'session-A');
});

test('a skipped chunk rejects pending history instead of delivering a corrupted transcript', { timeout: 5000 }, async t => {
  const client = launch(
    'send({type:"rpc_chunk",chunkId:"broken",index:0,count:3,byteLength:6,data:Buffer.from("abc").toString("base64")});'
    + 'send({type:"rpc_chunk",chunkId:"broken",index:2,count:3,byteLength:6,data:Buffer.from("def").toString("base64")});');
  t.after(() => client.dispose());
  await client.ready;
  await assert.rejects(client.request('get_messages'), /Interrupted NPi chunk sequence/);
});

test('child exit rejects in-flight requests so the host can recover rather than hang', { timeout: 5000 }, async t => {
  const client = launch('setTimeout(()=>process.exit(7),30);');
  t.after(() => client.dispose());
  await client.ready;
  const closed = once(client, 'close');
  await assert.rejects(client.request('get_state'), /NPi exited \(7\)/);
  assert.equal((await closed)[0].code, 7);
});
