import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const INVENTORY_DOMAIN = Buffer.from("kilogram:sync-inventory-signature:cbor-v2\0", "utf8");
const DIFF_DOMAIN = Buffer.from("kilogram:sync-diff-signature:cbor-v2\0", "utf8");

function fail(message) { throw new Error(message); }
function assert(condition, message) { if (!condition) fail(message); }
function head(major, value) {
  assert(Number.isSafeInteger(value) && value >= 0, "invalid CBOR argument");
  const prefix = major << 5;
  if (value <= 23) return Buffer.from([prefix | value]);
  if (value <= 0xff) return Buffer.from([prefix | 24, value]);
  if (value <= 0xffff) {
    const result = Buffer.alloc(3); result[0] = prefix | 25; result.writeUInt16BE(value, 1); return result;
  }
  if (value <= 0xffffffff) {
    const result = Buffer.alloc(5); result[0] = prefix | 26; result.writeUInt32BE(value, 1); return result;
  }
  const result = Buffer.alloc(9); result[0] = prefix | 27; result.writeBigUInt64BE(BigInt(value), 1); return result;
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
function signed(domain, content, privateKey) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, privateKey);
  assert(verifyEd25519(null, preimage, createPublicKey(privateKey), signature), "self signature failed");
  return { content, preimage, signature, envelope: array([uint(VERSION), content, bytes(signature)]) };
}
function signedExpected(domain, value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: domain.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
  };
}
function objectExpected(value) { return { cbor_hex: value.toString("hex") }; }
function nestedExpected(value, nested, source) {
  assert(value.subarray(value.length - nested.length).equals(nested), "nested object is not the exact suffix");
  return {
    prefix_hex: value.subarray(0, value.length - nested.length).toString("hex"),
    nested_source: source,
    encoded_length: value.length,
  };
}
function frame(variant, payload) { return array([uint(VERSION), uint(variant), bytes(payload)]); }

function generate(history, legacy, events, input) {
  const requesterPrivate = privateFromSeed(fromHex(input.requester_device_secret_key_hex, 32, "requester seed"));
  const responderPrivate = privateFromSeed(fromHex(input.responder_device_secret_key_hex, 32, "responder seed"));
  const requester = publicBytes(requesterPrivate);
  const responder = publicBytes(responderPrivate);
  const conversation = fromHex(legacy.expected.conversation_id_hex, 32, "legacy conversation ID");
  const binding = fromHex(input.session_binding_hex, 32, "session binding");
  const legacyKey = fromHex(history.expected.legacy_key_cbor_hex, undefined, "legacy typed key");
  const legacyCarrier = fromHex(history.expected.legacy_carrier_cbor_hex, undefined, "legacy carrier");

  assert(requester.toString("hex") === events.expected.author_device_id_hex,
    "requester seed does not match the canonical event author Device");

  const inventoryContent = array([
    uint(VERSION), bytes(conversation), bytes(requester), bytes(binding), array([bytes(legacyKey)]),
  ]);
  const inventory = signed(INVENTORY_DOMAIN, inventoryContent, requesterPrivate);
  const eventBatch = array([uint(VERSION), bytes(conversation), array([bytes(legacyCarrier)])]);
  const diffContent = array([
    uint(VERSION), bytes(conversation), bytes(responder), bytes(binding),
    array([bytes(legacyKey)]), array([]), uint(0),
  ]);
  const diff = signed(DIFF_DOMAIN, diffContent, responderPrivate);
  const complete = array([uint(VERSION), bytes(conversation), array([bytes(legacyKey)]), uint(0)]);
  const pause = array([uint(VERSION), bytes(conversation)]);
  const paused = array([uint(VERSION), bytes(conversation)]);
  const rejected = array([uint(VERSION), bytes(conversation), uint(1)]);

  return {
    requester_device_id_hex: requester.toString("hex"),
    responder_device_id_hex: responder.toString("hex"),
    conversation_id_hex: conversation.toString("hex"),
    session_binding_hex: binding.toString("hex"),
    inventory: signedExpected(INVENTORY_DOMAIN, inventory),
    event_batch: nestedExpected(
      eventBatch,
      legacyCarrier,
      "CANONICAL-SYNC-HISTORY-V2-VECTORS.json#/expected/legacy_carrier_cbor_hex",
    ),
    diff: signedExpected(DIFF_DOMAIN, diff),
    complete: objectExpected(complete),
    pause: objectExpected(pause),
    paused: objectExpected(paused),
    rejected: objectExpected(rejected),
    client_frames: {
      inventory_cbor_hex: frame(1, inventory.envelope).toString("hex"),
      events: nestedExpected(
        frame(2, eventBatch),
        eventBatch,
        "CANONICAL-SYNC-V2-VECTORS.json#/expected/event_batch",
      ),
      pause_cbor_hex: frame(3, pause).toString("hex"),
    },
    server_frames: {
      diff_cbor_hex: frame(1, diff.envelope).toString("hex"),
      complete_cbor_hex: frame(2, complete).toString("hex"),
      rejected_cbor_hex: frame(3, rejected).toString("hex"),
      paused_cbor_hex: frame(4, paused).toString("hex"),
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
    else if (["--schema", "--vectors", "--history-vectors", "--legacy-vectors", "--event-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const name of ["history-vectors", "legacy-vectors", "event-vectors"]) {
    assert(result[name], `--${name} is required`);
  }
  if (!result.emit) { assert(result.schema, "--schema is required"); assert(result.vectors, "--vectors is required"); }
  return result;
}

const options = parse(process.argv.slice(2));
const history = JSON.parse(readFileSync(options["history-vectors"], "utf8"));
const legacy = JSON.parse(readFileSync(options["legacy-vectors"], "utf8"));
const events = JSON.parse(readFileSync(options["event-vectors"], "utf8"));
const defaultInput = {
  requester_device_secret_key_hex: events.input.author_device_secret_key_hex,
  responder_device_secret_key_hex: "4142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f60",
  session_binding_hex: "f0e0d0c0b0a09080706050403020100000102030405060708090a0b0c0d0e0f0",
};

if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    milestone: "M1.1.54",
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(history, legacy, events, defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.54" &&
  schema.status === "history-compatible-frames-verified-session-and-capability-pending" &&
  schema.changes_active_wire === false, "sync v2 schema header changed");
assert(vectors.schema_version === 1 && vectors.milestone === "M1.1.54" &&
  vectors.profile === schema.profile && vectors.activation_status === "not-active", "sync v2 vector header changed");
equalJson(generate(history, legacy, events, vectors.input), vectors.expected, "independent sync v2 vector");

if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.input.session_binding_hex = `00${changed.input.session_binding_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(history, legacy, events, changed.input), changed.expected, "changed binding"); } catch { rejected = true; }
  assert(rejected, "changed session binding was accepted");
  process.stdout.write("canonical_sync_v2_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_sync_v2_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("typed_history_carriers=verified\n");
  process.stdout.write("active_runtime_changed=false\n");
}
