// Deliberately small, unoptimized BLAKE3 hash/derive-key reference for
// conformance checks only. Ported from the official BLAKE3 reference algorithm;
// hash mode is validated by verify-kilogram-blake3-conformance.mjs against
// official vectors, while derive-key mode is cross-checked by canonical KDF
// vectors against the independent Rust implementation.

const IV = Uint32Array.from([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
]);
const PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const BLOCK_LEN = 64;
const CHUNK_LEN = 1024;
const CHUNK_START = 1;
const CHUNK_END = 2;
const PARENT = 4;
const ROOT = 8;
const DERIVE_KEY_CONTEXT = 32;
const DERIVE_KEY_MATERIAL = 64;

function rotr(value, count) {
  return ((value >>> count) | (value << (32 - count))) >>> 0;
}

function add(left, right) {
  return (left + right) >>> 0;
}

function g(state, a, b, c, d, mx, my) {
  state[a] = add(add(state[a], state[b]), mx);
  state[d] = rotr((state[d] ^ state[a]) >>> 0, 16);
  state[c] = add(state[c], state[d]);
  state[b] = rotr((state[b] ^ state[c]) >>> 0, 12);
  state[a] = add(add(state[a], state[b]), my);
  state[d] = rotr((state[d] ^ state[a]) >>> 0, 8);
  state[c] = add(state[c], state[d]);
  state[b] = rotr((state[b] ^ state[c]) >>> 0, 7);
}

function round(state, message) {
  g(state, 0, 4, 8, 12, message[0], message[1]);
  g(state, 1, 5, 9, 13, message[2], message[3]);
  g(state, 2, 6, 10, 14, message[4], message[5]);
  g(state, 3, 7, 11, 15, message[6], message[7]);
  g(state, 0, 5, 10, 15, message[8], message[9]);
  g(state, 1, 6, 11, 12, message[10], message[11]);
  g(state, 2, 7, 8, 13, message[12], message[13]);
  g(state, 3, 4, 9, 14, message[14], message[15]);
}

function permute(message) {
  return Uint32Array.from(PERMUTATION, (index) => message[index]);
}

function compress(chainingValue, blockWords, counter, blockLength, flags) {
  const count = BigInt(counter);
  const state = new Uint32Array(16);
  state.set(chainingValue, 0);
  state.set(IV.subarray(0, 4), 8);
  state[12] = Number(count & 0xffffffffn) >>> 0;
  state[13] = Number((count >> 32n) & 0xffffffffn) >>> 0;
  state[14] = blockLength >>> 0;
  state[15] = flags >>> 0;
  let message = Uint32Array.from(blockWords);
  for (let index = 0; index < 7; index += 1) {
    round(state, message);
    if (index !== 6) message = permute(message);
  }
  const output = new Uint32Array(16);
  for (let index = 0; index < 8; index += 1) {
    output[index] = (state[index] ^ state[index + 8]) >>> 0;
    output[index + 8] = (state[index + 8] ^ chainingValue[index]) >>> 0;
  }
  return output;
}

function blockWords(bytes) {
  const padded = Buffer.alloc(BLOCK_LEN);
  Buffer.from(bytes).copy(padded);
  const words = new Uint32Array(16);
  for (let index = 0; index < 16; index += 1) words[index] = padded.readUInt32LE(index * 4);
  return words;
}

function wordsBytes(words) {
  const result = Buffer.alloc(words.length * 4);
  for (let index = 0; index < words.length; index += 1) result.writeUInt32LE(words[index], index * 4);
  return result;
}

function output(inputCv, words, counter, blockLength, flags) {
  return {
    chainingValue() {
      return compress(inputCv, words, counter, blockLength, flags).slice(0, 8);
    },
    rootBytes(length) {
      const blocks = [];
      for (let outputCounter = 0; blocks.length * 64 < length; outputCounter += 1) {
        blocks.push(wordsBytes(compress(inputCv, words, BigInt(outputCounter), blockLength, flags | ROOT)));
      }
      return Buffer.concat(blocks).subarray(0, length);
    },
  };
}

function chunkOutput(input, chunkCounter, keyWords = IV, baseFlags = 0) {
  const bytes = Buffer.from(input);
  let cv = Uint32Array.from(keyWords);
  let offset = 0;
  let blocksCompressed = 0;
  while (bytes.length - offset > BLOCK_LEN) {
    const flags = baseFlags | (blocksCompressed === 0 ? CHUNK_START : 0);
    cv = compress(cv, blockWords(bytes.subarray(offset, offset + BLOCK_LEN)),
      BigInt(chunkCounter), BLOCK_LEN, flags).slice(0, 8);
    offset += BLOCK_LEN;
    blocksCompressed += 1;
  }
  const finalBlock = bytes.subarray(offset);
  const flags = baseFlags | CHUNK_END | (blocksCompressed === 0 ? CHUNK_START : 0);
  return output(cv, blockWords(finalBlock), BigInt(chunkCounter), finalBlock.length, flags);
}

function parentOutput(left, right, keyWords = IV, baseFlags = 0) {
  const words = new Uint32Array(16);
  words.set(left, 0);
  words.set(right, 8);
  return output(keyWords, words, 0n, BLOCK_LEN, baseFlags | PARENT);
}

function blake3Internal(input, length, keyWords, baseFlags) {
  if (!Number.isSafeInteger(length) || length < 0) throw new Error("invalid BLAKE3 output length");
  const bytes = Buffer.from(input);
  const chunkCount = Math.max(1, Math.ceil(bytes.length / CHUNK_LEN));
  const stack = [];
  for (let chunkIndex = 0; chunkIndex + 1 < chunkCount; chunkIndex += 1) {
    const start = chunkIndex * CHUNK_LEN;
    let cv = chunkOutput(bytes.subarray(start, start + CHUNK_LEN), BigInt(chunkIndex),
      keyWords, baseFlags).chainingValue();
    let totalChunks = chunkIndex + 1;
    while (totalChunks % 2 === 0) {
      const left = stack.pop();
      if (left === undefined) throw new Error("invalid BLAKE3 tree stack");
      cv = parentOutput(left, cv, keyWords, baseFlags).chainingValue();
      totalChunks /= 2;
    }
    stack.push(cv);
  }
  const lastIndex = chunkCount - 1;
  let root = chunkOutput(bytes.subarray(lastIndex * CHUNK_LEN), BigInt(lastIndex),
    keyWords, baseFlags);
  while (stack.length > 0) {
    root = parentOutput(stack.pop(), root.chainingValue(), keyWords, baseFlags);
  }
  return root.rootBytes(length);
}

function keyWords(bytes) {
  const input = Buffer.from(bytes);
  if (input.length !== 32) throw new Error("BLAKE3 key must contain 32 bytes");
  const words = new Uint32Array(8);
  for (let index = 0; index < words.length; index += 1) {
    words[index] = input.readUInt32LE(index * 4);
  }
  return words;
}

export function blake3Hash(input, length = 32) {
  return blake3Internal(input, length, IV, 0);
}

export function blake3DeriveKey(context, material, length = 32) {
  if (typeof context !== "string") throw new Error("BLAKE3 derive-key context must be text");
  const contextKey = blake3Internal(Buffer.from(context, "utf8"), 32, IV, DERIVE_KEY_CONTEXT);
  return blake3Internal(material, length, keyWords(contextKey), DERIVE_KEY_MATERIAL);
}
