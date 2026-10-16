import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const SIGNATURE_DOMAIN = Buffer.from("kilogram:sync-session-checkpoint-signature:cbor-v2\0", "utf8");
const ID_DOMAIN = Buffer.from("kilogram:sync-session-checkpoint-id:cbor-v2\0", "utf8");
const PROTOCOL_IDENTIFIER = Buffer.from("kilogram/m1/sync/cbor/2", "utf8");

function fail(message) { throw new Error(message); }
function assert(condition, message) { if (!condition) fail(message); }
function head(major, value) {
  const number = BigInt(value);
  assert(number >= 0n, "negative CBOR argument");
  if (number < 24n) return Buffer.from([(major << 5) | Number(number)]);
  if (number <= 0xffn) return Buffer.from([(major << 5) | 24, Number(number)]);
  if (number <= 0xffffn) {
    const output = Buffer.alloc(3); output[0] = (major << 5) | 25; output.writeUInt16BE(Number(number), 1); return output;
  }
  if (number <= 0xffffffffn) {
    const output = Buffer.alloc(5); output[0] = (major << 5) | 26; output.writeUInt32BE(Number(number), 1); return output;
  }
  const output = Buffer.alloc(9); output[0] = (major << 5) | 27; output.writeBigUInt64BE(number, 1); return output;
}
const uint = (value) => head(0, value);
const bytes = (value) => Buffer.concat([head(2, value.length), value]);
const array = (items) => Buffer.concat([head(4, items.length), ...items]);
function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function privateFromSeed(seed) {
  return createPrivateKey({ key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8" });
}
function publicBytes(privateKey) {
  const spki = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI prefix");
  return spki.subarray(SPKI_PREFIX.length);
}
function signedCheckpoint(content, privateKey) {
  const preimage = Buffer.concat([SIGNATURE_DOMAIN, content]);
  const signature = signEd25519(null, preimage, privateKey);
  assert(verifyEd25519(null, preimage, createPublicKey(privateKey), signature), "checkpoint self signature failed");
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  const id = blake3Hash(Buffer.concat([ID_DOMAIN, envelope]));
  return { content, preimage, signature, envelope, id };
}
function optionalId(id) { return id === undefined ? array([]) : array([bytes(id)]); }
function checkpointContent(input) {
  return array([
    uint(VERSION), bytes(PROTOCOL_IDENTIFIER), bytes(input.conversation), bytes(input.local), bytes(input.peer),
    bytes(input.binding), uint(input.sequence), uint(input.round), optionalId(input.predecessor),
    array(input.received.map(bytes)), array(input.sent.map(bytes)),
    uint(input.localMore ? 1 : 0), uint(input.peerMore ? 1 : 0),
    uint(input.nextAction), uint(input.paused ? 1 : 0),
  ]);
}
function expected(value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: SIGNATURE_DOMAIN.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
    checkpoint_id_domain_hex: ID_DOMAIN.toString("hex"),
    checkpoint_id_hex: value.id.toString("hex"),
  };
}

function generate(history, sync, input) {
  const localPrivate = privateFromSeed(fromHex(input.local_device_secret_key_hex, 32, "local Device seed"));
  const local = publicBytes(localPrivate);
  const peer = fromHex(sync.expected.responder_device_id_hex, 32, "peer Device ID");
  const conversation = fromHex(sync.expected.conversation_id_hex, 32, "conversation ID");
  const binding = fromHex(sync.expected.session_binding_hex, 32, "session binding");
  const received = [fromHex(history.expected.legacy_key_cbor_hex, undefined, "received typed Event key")];
  const sent = [];
  assert(local.toString("hex") === sync.expected.requester_device_id_hex,
    "local seed does not match sync requester Device");

  const first = signedCheckpoint(checkpointContent({
    conversation, local, peer, binding, sequence: 1, round: 1,
    predecessor: undefined, received, sent, localMore: true, peerMore: true,
    nextAction: 4, paused: false,
  }), localPrivate);
  const second = signedCheckpoint(checkpointContent({
    conversation, local, peer, binding, sequence: 2, round: 2,
    predecessor: first.id, received, sent, localMore: true, peerMore: true,
    nextAction: 1, paused: false,
  }), localPrivate);
  const paused = signedCheckpoint(checkpointContent({
    conversation, local, peer, binding, sequence: 3, round: 2,
    predecessor: second.id, received, sent, localMore: true, peerMore: true,
    nextAction: 1, paused: true,
  }), localPrivate);
  const resumed = signedCheckpoint(checkpointContent({
    conversation, local, peer, binding, sequence: 4, round: 2,
    predecessor: paused.id, received, sent, localMore: true, peerMore: true,
    nextAction: 1, paused: false,
  }), localPrivate);
  return {
    local_device_id_hex: local.toString("hex"),
    peer_device_id_hex: peer.toString("hex"),
    protocol_identifier_utf8: PROTOCOL_IDENTIFIER.toString("utf8"),
    protocol_identifier_hex: PROTOCOL_IDENTIFIER.toString("hex"),
    conversation_id_hex: conversation.toString("hex"),
    session_binding_hex: binding.toString("hex"),
    received_event_keys_cbor_hex: received.map((value) => value.toString("hex")),
    sent_event_keys_cbor_hex: sent.map((value) => value.toString("hex")),
    checkpoints: [first, second, paused, resumed].map(expected),
  };
}

function equalJson(actual, expectedValue, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expectedValue), `${label} diverged`);
}
function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--history-vectors", "--sync-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["history-vectors"], "--history-vectors is required");
  assert(result["sync-vectors"], "--sync-vectors is required");
  if (!result.emit) { assert(result.schema, "--schema is required"); assert(result.vectors, "--vectors is required"); }
  return result;
}

const options = parse(process.argv.slice(2));
const history = JSON.parse(readFileSync(options["history-vectors"], "utf8"));
const sync = JSON.parse(readFileSync(options["sync-vectors"], "utf8"));
const defaultInput = { local_device_secret_key_hex: sync.input.requester_device_secret_key_hex };

if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    milestone: "M1.1.55",
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(history, sync, defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.55" && schema.status === "typed-durable-checkpoint-verified-capability-pending" &&
  schema.changes_active_wire === false, "sync session schema header changed");
assert(vectors.schema_version === 1 && vectors.milestone === schema.milestone && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "sync session vector header changed");
equalJson(generate(history, sync, vectors.input), vectors.expected, "independent sync session v2 vector");

if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.input.local_device_secret_key_hex = `ff${changed.input.local_device_secret_key_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(history, sync, changed.input), changed.expected, "changed local key"); } catch { rejected = true; }
  assert(rejected, "changed local key was accepted");
  process.stdout.write("canonical_sync_session_v2_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_sync_session_v2_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_ed25519=verified\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("active_runtime_changed=false\n");
}
