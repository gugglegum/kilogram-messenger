import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  deviceListDigest: "kilogram:runtime-ipc-device-list-digest:cbor-v1\0",
  artifactDigest:
    "kilogram:runtime-ipc-own-device-announcement-artifact-digest:cbor-v1\0",
  policyId: "kilogram:runtime-ipc-own-device-announcement-policy-id:cbor-v1\0",
  snapshotId:
    "kilogram:runtime-ipc-own-device-announcement-snapshot-id:cbor-v1\0",
  requestDigest:
    "kilogram:runtime-ipc-own-device-announcement-request-digest:cbor-v1\0",
  responseSignature:
    "kilogram:runtime-ipc-own-device-announcement-response-signature:cbor-v1\0",
  responseId:
    "kilogram:runtime-ipc-own-device-announcement-response-id:cbor-v1\0",
}).map(([name, value]) => [name, Buffer.from(value)]));

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
const none = () => array([uint(0)]);

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
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
function publicFromBytes(raw) {
  return createPublicKey({ key: Buffer.concat([SPKI_PREFIX, raw]), format: "der", type: "spki" });
}
function sign(domain, content, signer) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer);
  assert(verifyEd25519(null, preimage, publicFromBytes(publicBytes(signer)), signature),
    "generated Ed25519 signature failed");
  return {
    content,
    preimage,
    signature,
    cbor: array([uint(VERSION), content, bytes(signature)]),
  };
}
function requestJson(value) {
  return {
    cbor_hex: value.toString("hex"),
    object_id_hex: domainHash(DOMAINS.requestDigest, value).toString("hex"),
  };
}
function responseJson(value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signing_bytes_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    object_id_hex: domainHash(DOMAINS.responseId, value.cbor).toString("hex"),
  };
}
function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((name) => [name, stable(value[name])]));
  }
  return value;
}
function equalJson(actual, expected, label) {
  const left = stable(actual); const right = stable(expected);
  if (JSON.stringify(left) === JSON.stringify(right)) return;
  const visit = (actualValue, expectedValue, path) => {
    if (JSON.stringify(actualValue) === JSON.stringify(expectedValue)) return undefined;
    if (actualValue !== null && expectedValue !== null &&
        typeof actualValue === "object" && typeof expectedValue === "object") {
      const names = [...new Set([...Object.keys(actualValue), ...Object.keys(expectedValue)])].sort();
      for (const name of names) {
        const mismatch = visit(actualValue[name], expectedValue[name], `${path}/${name}`);
        if (mismatch) return mismatch;
      }
    }
    return `${path}: actual=${JSON.stringify(actualValue)} expected=${JSON.stringify(expectedValue)}`;
  };
  fail(`${label} diverged at ${visit(left, right, "expected")}`);
}

function configuration(input, recipient, ticketDigest) {
  return array([
    uint(VERSION), bytes(recipient), uint(input.enabled ? 1 : 0),
    array([uint(input.recipient_ticket_bytes), bytes(ticketDigest)]),
    uint(input.interval_seconds), uint(input.validity_seconds),
    uint(input.retry_base_seconds), uint(input.retry_max_seconds),
    uint(input.allowed_network_mask),
  ]);
}

function generate(input) {
  const signer = privateKey(fromHex(input.device_secret_key_hex, 32, "Device secret"));
  const device = publicBytes(signer);
  const localDevice = fromHex(input.local_device_id_hex, 32, "local Device ID");
  assert(device.equals(localDevice), "Device secret does not match local Device ID");
  const account = fromHex(input.account_id_hex, 32, "Account ID");
  const descriptorId = fromHex(input.descriptor_id_hex, 32, "descriptor ID");
  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const roster = fromHex(input.current_device_list_signed_cbor_hex, undefined,
    "current Device list");
  const rosterDigest = domainHash(DOMAINS.deviceListDigest, roster);
  assert(rosterDigest.toString("hex") === input.device_list_digest_hex,
    "current Device-list digest changed");
  const recipient = fromHex(input.recipient_device_id_hex, 32, "recipient Device ID");
  assert(!recipient.equals(device), "recipient must be a sibling Device");
  const ticket = fromHex(input.recipient_ticket_hex, undefined, "recipient ticket");
  assert(ticket.length === input.recipient_ticket_bytes, "recipient ticket length changed");
  const ticketDigest = domainHash(DOMAINS.artifactDigest, ticket);
  assert(ticketDigest.toString("hex") === input.recipient_ticket_digest_hex,
    "recipient ticket digest changed");
  const config = configuration(input, recipient, ticketDigest);
  const generation = 1;
  const policyPreimage = array([
    uint(VERSION), bytes(account), bytes(device), uint(input.authority_revision),
    bytes(rosterDigest), uint(generation), none(),
    uint(input.configure_issued_at_unix_seconds), config,
  ]);
  const policyId = domainHash(DOMAINS.policyId, policyPreimage);
  const policy = array([
    uint(VERSION), config, uint(generation), none(), bytes(policyId),
    uint(input.configure_issued_at_unix_seconds), uint(2), none(), none(), none(),
    uint(0), none(), none(),
  ]);

  const configureRequestId = fromHex(input.configure_request_id_hex, 32,
    "configure request ID");
  const configureRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(configureRequestId), bytes(bearer),
    uint(input.configure_issued_at_unix_seconds),
    uint(input.configure_expires_at_unix_seconds),
    array([uint(0), bytes(roster), none(), config]),
  ]);
  const configureDigest = domainHash(DOMAINS.requestDigest, configureRequest);
  const configureResult = array([
    uint(0), uint(0), uint(input.authority_revision), bytes(rosterDigest),
    uint(1), policy, uint(0), uint(0), uint(0),
  ]);
  const configureContent = array([
    uint(VERSION), bytes(descriptorId), bytes(configureRequestId), bytes(configureDigest),
    bytes(device), configureResult,
  ]);
  const configureResponse = sign(DOMAINS.responseSignature, configureContent, signer);

  const snapshotPreimage = array([
    uint(VERSION), bytes(account), bytes(device), uint(input.authority_revision),
    bytes(rosterDigest), uint(1), array([policy]),
  ]);
  const snapshotId = domainHash(DOMAINS.snapshotId, snapshotPreimage);
  const statusRequestId = fromHex(input.status_request_id_hex, 32, "status request ID");
  const statusRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(statusRequestId), bytes(bearer),
    uint(input.status_issued_at_unix_seconds), uint(input.status_expires_at_unix_seconds),
    array([uint(1), bytes(roster)]),
  ]);
  const statusDigest = domainHash(DOMAINS.requestDigest, statusRequest);
  const statusResult = array([
    uint(1), uint(input.authority_revision), bytes(rosterDigest), uint(1), bytes(snapshotId),
    array([policy]), uint(0), uint(0), uint(1),
  ]);
  const statusContent = array([
    uint(VERSION), bytes(descriptorId), bytes(statusRequestId), bytes(statusDigest),
    bytes(device), statusResult,
  ]);
  const statusResponse = sign(DOMAINS.responseSignature, statusContent, signer);

  const ipc = Buffer.concat([
    configureRequest, configureResponse.cbor, statusRequest, statusResponse.cbor,
  ]);
  assert(ipc.indexOf(ticket) === -1, "recipient ticket plaintext leaked into IPC");

  return {
    policy_id_hex: policyId.toString("hex"),
    snapshot_id_hex: snapshotId.toString("hex"),
    configure: {
      request: requestJson(configureRequest),
      response: responseJson(configureResponse),
    },
    status: {
      request: requestJson(statusRequest),
      response: responseJson(statusResponse),
    },
  };
}

function argumentsMap(argv) {
  const output = new Map();
  for (let index = 2; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === "--self-test") { output.set(key, true); continue; }
    const value = argv[index + 1];
    assert(value !== undefined, `missing value after ${key}`);
    output.set(key, value); index += 1;
  }
  return output;
}

const args = argumentsMap(process.argv);
for (const required of ["--schema", "--vectors"]) {
  assert(args.has(required), `missing ${required}`);
}
const schema = JSON.parse(readFileSync(args.get("--schema"), "utf8"));
const vectors = JSON.parse(readFileSync(args.get("--vectors"), "utf8"));
assert(schema.milestone === "M1.1.43" && schema.status === "candidate-not-active",
  "runtime IPC schema is not the expected inactive M1.1.43 candidate");
assert(schema.domains.length === 67 && schema.objects.length === 33 &&
  schema.covered_commands.length === 30 && schema.covered_results.length === 29 &&
  schema.semantic_rules.length === 136,
  "runtime IPC schema registry counts changed");
assert(vectors.schema_version === 1 && vectors.milestone === "M1.1.40" &&
  vectors.status === "candidate-not-active", "own-device announcement vector header changed");
for (const [name, domain] of Object.entries({
  artifact_digest: DOMAINS.artifactDigest,
  policy_id: DOMAINS.policyId,
  snapshot_id: DOMAINS.snapshotId,
  request_digest: DOMAINS.requestDigest,
  response_signature: DOMAINS.responseSignature,
  response_id: DOMAINS.responseId,
})) {
  assert(vectors.domains[name] === domain.subarray(0, domain.length - 1).toString("utf8"),
    `${name} domain changed`);
}
equalJson(generate(vectors.input), vectors.expected,
  "canonical runtime IPC own-device announcement reference");

if (args.has("--self-test")) {
  const changed = structuredClone(vectors.input);
  changed.interval_seconds += 1;
  assert(generate(changed).configure.request.cbor_hex !== vectors.expected.configure.request.cbor_hex,
    "interval mutation did not change canonical bytes");
  process.stdout.write("canonical_runtime_ipc_own_device_announcement_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_runtime_ipc_own_device_announcement_reference=verified\n");
