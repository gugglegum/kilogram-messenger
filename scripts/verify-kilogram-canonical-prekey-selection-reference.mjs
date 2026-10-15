import { readFileSync } from "node:fs";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const POOL_ID_DOMAIN = Buffer.from("kilogram:prekey-pool-id:cbor-v1\0", "utf8");
const SELECTION_DOMAIN = Buffer.from("kilogram:prekey-selection:cbor-v1\0", "utf8");

function fail(message) { throw new Error(message); }
function assert(condition, message) { if (!condition) fail(message); }
function fromHex(value, bytes, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hex`);
  const result = Buffer.from(value, "hex");
  if (bytes !== undefined) assert(result.length === bytes, `${label} must contain ${bytes} bytes`);
  return result;
}
function head(major, value) {
  const number = BigInt(value);
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

function generate(ratchet, input) {
  const initiator = fromHex(input.initiator_device_id_hex, 32, "initiator Device ID");
  const pool = fromHex(input.signed_prekey_pool_cbor_hex, undefined, "signed prekey pool");
  assert(pool.toString("hex") === ratchet.expected.prekey_pool.signed_cbor_hex,
    "selection vector does not embed the exact ratchet pool vector");
  const poolId = blake3Hash(Buffer.concat([POOL_ID_DOMAIN, pool]));
  assert(poolId.toString("hex") === ratchet.expected.prekey_pool_id_hex,
    "independent prekey pool ID mismatch");
  const selectionInput = array([uint(VERSION), bytes(initiator), bytes(poolId)]);
  const digest = blake3Hash(Buffer.concat([SELECTION_DOMAIN, selectionInput]));
  const prefix = digest.readBigUInt64BE(0);
  const sequences = ratchet.input.prekey_sequences;
  assert(Array.isArray(sequences) && sequences.length > 0 && sequences.length <= 64,
    "invalid ratchet prekey sequence inventory");
  const selectedIndex = Number(prefix % BigInt(sequences.length));
  const selectedSequence = sequences[selectedIndex];
  const transcript = array([
    uint(VERSION), bytes(initiator), bytes(pool), bytes(digest),
    uint(selectedIndex), uint(selectedSequence),
  ]);
  return {
    pool_id_hex: poolId.toString("hex"),
    selection_domain_hex: SELECTION_DOMAIN.toString("hex"),
    selection_input_cbor_hex: selectionInput.toString("hex"),
    selection_digest_hex: digest.toString("hex"),
    digest_prefix_u64_be: prefix.toString(10),
    selected_index: selectedIndex,
    selected_sequence: selectedSequence,
    selected_bundle_cbor_hex: ratchet.expected.prekey_bundles[selectedIndex].signed_cbor_hex,
    transcript_cbor_hex: transcript.toString("hex"),
  };
}

function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}

function parse(argv) {
  const value = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") value.emit = true;
    else if (argument === "--self-test") value.selfTest = true;
    else if (["--schema", "--vectors", "--ratchet-vectors"].includes(argument)) {
      value[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(value["ratchet-vectors"], "--ratchet-vectors is required");
  if (!value.emit) { assert(value.schema, "--schema is required"); assert(value.vectors, "--vectors is required"); }
  return value;
}

const options = parse(process.argv.slice(2));
const ratchet = JSON.parse(readFileSync(options["ratchet-vectors"], "utf8"));
const defaultInput = {
  initiator_device_id_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  signed_prekey_pool_cbor_hex: ratchet.expected.prekey_pool.signed_cbor_hex,
};

if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(ratchet, defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.11" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "prekey selection schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "prekey selection vector header changed");
equalJson(generate(ratchet, vectors.input), vectors.expected, "independent prekey selection vector");

if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.input.initiator_device_id_hex = `ff${changed.input.initiator_device_id_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(ratchet, changed.input), changed.expected, "changed initiator"); } catch { rejected = true; }
  assert(rejected, "changed initiator was accepted");
  process.stdout.write("canonical_prekey_selection_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_prekey_selection_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("selection_integer_endianness=big\n");
  process.stdout.write("active_runtime_changed=false\n");
}
