import assert from 'node:assert/strict';
import { test } from 'node:test';
import { checkInvocation } from './wasmtime.mjs';

const unreachable = 'failed to invoke `indexed`\n    2: wasm trap: wasm `unreachable` instruction executed\n';

function checkTrap(overrides) {
    checkInvocation({ status: 134, stdout: '', stderr: unreachable, ...overrides }, 'indexed', null);
}

test('accepts the unreachable cause from the selected export', () => {
    checkTrap({});
});

test('rejects fuel exhaustion, missing exports and unexpected process outcomes', () => {
    for (const overrides of [
        { stderr: 'failed to invoke `indexed`\n    2: wasm trap: all fuel consumed by WebAssembly\n' },
        { stderr: 'failed to find function export `indexed`\n' },
        { stderr: 'failed to invoke `other`\n    2: wasm trap: wasm `unreachable` instruction executed\n' },
        { status: null },
        { status: 0 },
        { stdout: '42\n' },
    ]) {
        assert.throws(() => checkTrap(overrides), /expected an unreachable trap/);
    }
});

test('scalar cases require both the expected value and a successful exit', () => {
    checkInvocation({ status: 0, stdout: '65\n', stderr: '' }, 'scalar', '65');
    assert.throws(() => checkInvocation({ status: 0, stdout: '64\n', stderr: '' }, 'scalar', '65'),
        /unexpected result/);
    assert.throws(() => checkInvocation({ status: 1, stdout: '65\n', stderr: '' }, 'scalar', '65'),
        /Wasmtime failed/);
});
