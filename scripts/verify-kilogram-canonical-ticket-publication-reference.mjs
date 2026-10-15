import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import { blake3DeriveKey, blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = {
  channelId: Buffer.from("kilogram:ticket-publication-capability-channel:v1\0"),
  authorizationSignature: Buffer.from(
    "kilogram:ticket-publication-write-authorization:cbor-v1\0",
  ),
};
const CONTEXTS = {
  legacy: "kilogram ticket publication per-peer write capability v1",
  rotated: "kilogram ticket publication per-peer rotated write capability v1",
};

function fail(message) { throw new Error(message); }
function assert(condition, message) { if (!condition) fail(message); }
function head(major, value) {
  const number = BigInt(value);
  assert(number >= 0n, "negative CBOR argument");
  if (number < 24n) return Buffer.from([(major << 5) | Number(number)]);
  if (number <= 0xffn) return Buffer.from([(major << 5) | 24, Number(number)]);
  if (number <= 0xffffn) {
    const output = Buffer.alloc(3); output[0] = (major << 5) | 25;
    output.writeUInt16BE(Number(number), 1); return output;
  }
  if (number <= 0xffffffffn) {
    const output = Buffer.alloc(5); output[0] = (major << 5) | 26;
    output.writeUInt32BE(Number(number), 1); return output;
  }
  const output = Buffer.alloc(9); output[0] = (major << 5) | 27;
  output.writeBigUInt64BE(number, 1); return output;
}
const uint = (value) => head(0, value);
const bytes = (value) => Buffer.concat([head(2, value.length), value]);
const array = (values) => Buffer.concat([head(4, values.length), ...values]);
function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function u64be(value) {
  const output = Buffer.alloc(8); output.writeBigUInt64BE(BigInt(value)); return output;
}
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function privateKey(seed) {
  return createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8",
  });
}
function publicBytes(key) {
  const der = createPublicKey(key).export({ format: "der", type: "spki" });
  assert(der.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI");
  return der.subarray(SPKI_PREFIX.length);
}
function derivedSeed(deviceSecret, peerScope, epoch) {
  if (BigInt(epoch) === 0n) {
    return blake3DeriveKey(CONTEXTS.legacy, Buffer.concat([deviceSecret, peerScope]));
  }
  return blake3DeriveKey(CONTEXTS.rotated,
    Buffer.concat([deviceSecret, peerScope, Buffer.from([0]), u64be(epoch)]));
}
function capability(deviceSecret, peerScope, epoch) {
  const seed = derivedSeed(deviceSecret, peerScope, epoch);
  const key = privateKey(seed);
  const writeKey = publicBytes(key);
  const channelId = domainHash(DOMAINS.channelId, writeKey);
  const cbor = array([uint(VERSION), bytes(writeKey), bytes(channelId)]);
  return { seed, key, writeKey, channelId, cbor };
}
function capabilityJson(value) {
  return {
    write_key_hex: value.writeKey.toString("hex"),
    channel_id_hex: value.channelId.toString("hex"),
    cbor_hex: value.cbor.toString("hex"),
  };
}
function generate(input) {
  const deviceSecret = fromHex(input.device_secret_hex, 32, "device secret");
  const peerScope = fromHex(input.peer_scope_hex, undefined, "peer scope");
  assert(peerScope.length > 0 && peerScope.length <= 4096, "peer scope is outside bounds");
  const body = fromHex(input.body_hex, undefined, "body");
  assert(body.length > 0 && body.length <= 16 * 1024 * 1024, "body is outside bounds");
  assert(BigInt(input.generation) > 0n, "generation must be positive");
  const legacy = capability(deviceSecret, peerScope, 0);
  const rotated = capability(deviceSecret, peerScope, input.rotated_epoch);
  const bodyDigest = blake3Hash(body);
  const content = array([
    uint(VERSION), bytes(rotated.cbor), uint(input.generation), uint(body.length), bytes(bodyDigest),
  ]);
  const preimage = Buffer.concat([DOMAINS.authorizationSignature, content]);
  const signature = signEd25519(null, preimage, rotated.key);
  const publicKey = createPublicKey({
    key: Buffer.concat([SPKI_PREFIX, rotated.writeKey]), format: "der", type: "spki",
  });
  assert(verifyEd25519(null, preimage, publicKey, signature), "authorization signature failed");
  const signed = array([uint(VERSION), bytes(content), bytes(signature)]);
  return {
    domains: {
      channel_id_hex: DOMAINS.channelId.toString("hex"),
      authorization_signature_hex: DOMAINS.authorizationSignature.toString("hex"),
      legacy_derivation_context: CONTEXTS.legacy,
      rotated_derivation_context: CONTEXTS.rotated,
    },
    legacy_capability: capabilityJson(legacy),
    rotated_capability: capabilityJson(rotated),
    authorization: {
      body_digest_hex: bodyDigest.toString("hex"),
      content_cbor_hex: content.toString("hex"),
      signing_preimage_hex: preimage.toString("hex"),
      signature_hex: signature.toString("hex"),
      signed_cbor_hex: signed.toString("hex"),
    },
  };
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  device_secret_hex: "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f",
  peer_scope_hex: "303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f",
  rotated_epoch: 7,
  generation: 9,
  body_hex: Buffer.from("opaque ticket publication body v1", "utf8").toString("hex"),
};

const options = parse(process.argv.slice(2));
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.20" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.activation.current_runtime_encoding === "postcard-and-lowercase-hex-http-headers" &&
  schema.activation.current_runtime_changed === false, "active ticket publication changed");
equalJson(generate(vectors.input), vectors.expected, "canonical ticket publication vectors");

if (options.selfTest) {
  const changedBody = structuredClone(vectors.input);
  changedBody.body_hex = `ff${changedBody.body_hex.slice(2)}`;
  assert(generate(changedBody).authorization.signature_hex !==
    vectors.expected.authorization.signature_hex, "changed body preserved authorization");
  const changedEpoch = structuredClone(vectors.input);
  changedEpoch.rotated_epoch += 1;
  assert(generate(changedEpoch).rotated_capability.write_key_hex !==
    vectors.expected.rotated_capability.write_key_hex, "changed epoch preserved capability");
  process.stdout.write("changed_body=rejected\n");
  process.stdout.write("changed_epoch=rejected\n");
  process.stdout.write("canonical_ticket_publication_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3_hash_and_derive_key=verified\n");
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("independent_ed25519=verified\n");
process.stdout.write("canonical_ticket_publication_reference=verified\n");
