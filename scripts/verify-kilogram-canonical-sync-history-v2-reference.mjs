import { readFileSync } from "node:fs";

const VERSION = 1;
const LEGACY = 1;
const CANONICAL = 2;

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
  fail("unneeded large CBOR argument");
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
const key = (namespace, id) => array([uint(VERSION), uint(namespace), bytes(id)]);
const carrier = (namespace, payload) => array([uint(VERSION), uint(namespace), bytes(payload)]);

function generate(legacy, canonical) {
  const legacyId = fromHex(legacy.expected.event_id_hex, 32, "legacy Event ID");
  const canonicalId = fromHex(canonical.expected.text_event.event_id_hex, 32, "canonical Event ID");
  const legacyPayload = fromHex(legacy.expected.bridge_cbor_hex, undefined, "legacy bridge");
  const canonicalPayload = fromHex(
    canonical.expected.authorized_text_event_cbor_hex,
    undefined,
    "canonical authorized event",
  );
  return {
    legacy_key_cbor_hex: key(LEGACY, legacyId).toString("hex"),
    canonical_key_cbor_hex: key(CANONICAL, canonicalId).toString("hex"),
    legacy_carrier_cbor_hex: carrier(LEGACY, legacyPayload).toString("hex"),
    canonical_carrier_cbor_hex: carrier(CANONICAL, canonicalPayload).toString("hex"),
  };
}

function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--legacy-vectors", "--event-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["legacy-vectors"], "--legacy-vectors is required");
  assert(result["event-vectors"], "--event-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const options = parse(process.argv.slice(2));
const legacy = JSON.parse(readFileSync(options["legacy-vectors"], "utf8"));
const canonical = JSON.parse(readFileSync(options["event-vectors"], "utf8"));
const expected = generate(legacy, canonical);

if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    milestone: "M1.1.53",
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    expected,
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.53" &&
  schema.status === "typed-history-carrier-verified-frames-pending" &&
  schema.changes_active_wire === false, "history-v2 schema header changed");
assert(schema.candidate_protocol_identifier === "kilogram/m1/sync/cbor/2" &&
  schema.candidate_identifier_allocated === true &&
  schema.candidate_identifier_registered === false &&
  schema.candidate_identifier_dialed === false, "history-v2 activation boundary changed");
assert(vectors.schema_version === VERSION && vectors.milestone === "M1.1.53" &&
  vectors.profile === schema.profile && vectors.activation_status === "not-active",
  "history-v2 vector header changed");
assert(JSON.stringify(vectors.expected) === JSON.stringify(expected),
  "independent history-v2 vector diverged");

if (options.selfTest) {
  const changed = structuredClone(vectors);
  changed.expected.legacy_key_cbor_hex = `00${changed.expected.legacy_key_cbor_hex.slice(2)}`;
  assert(JSON.stringify(changed.expected) !== JSON.stringify(expected),
    "changed history-v2 vector was accepted");
  process.stdout.write("canonical_sync_history_v2_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_sync_history_v2_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("typed_event_namespaces=2\n");
  process.stdout.write("active_runtime_changed=false\n");
}
