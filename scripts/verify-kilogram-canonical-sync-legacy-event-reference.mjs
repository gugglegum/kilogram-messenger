import { readFileSync } from "node:fs";

const VERSION = 1;

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

function encode(input, expected) {
  const active = fromHex(input.active_authorized_event_postcard_hex, undefined, "active event");
  assert(active.length > 0 && active.length <= 8388352, "active event length is outside the bridge bound");
  const encoded = array([
    uint(VERSION),
    bytes(fromHex(expected.conversation_id_hex, 32, "conversation ID")),
    bytes(fromHex(expected.event_id_hex, 32, "Event ID")),
    bytes(fromHex(expected.author_account_id_hex, 32, "author Account ID")),
    bytes(fromHex(expected.author_device_id_hex, 32, "author Device ID")),
    bytes(active),
  ]);
  assert(encoded.length <= 8388608, "bridge length exceeds the bound");
  return encoded;
}

function parse(argv) {
  const result = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result.schema && result.vectors, "--schema and --vectors are required");
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.schema_version === VERSION && schema.milestone === "M1.1.52" &&
  schema.status === "candidate-prerequisite-not-active" && schema.changes_active_wire === false,
  "legacy-event bridge schema header changed");
assert(vectors.schema_version === VERSION && vectors.milestone === "M1.1.52" &&
  vectors.profile === schema.profile && vectors.activation_status === "not-active",
  "legacy-event bridge vector header changed");
const encoded = encode(vectors.input, vectors.expected);
assert(encoded.toString("hex") === vectors.expected.bridge_cbor_hex,
  "independent legacy-event bridge encoding diverged");

if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.expected.event_id_hex = `00${changed.expected.event_id_hex.slice(2)}`;
  let rejected = false;
  try {
    assert(encode(changed.input, changed.expected).toString("hex") === changed.expected.bridge_cbor_hex,
      "changed Event ID");
  } catch { rejected = true; }
  assert(rejected, "changed Event ID was accepted");
  process.stdout.write("canonical_sync_legacy_event_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_sync_legacy_event_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("exact_active_bytes=preserved\n");
  process.stdout.write("active_runtime_changed=false\n");
}
