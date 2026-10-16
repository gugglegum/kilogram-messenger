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
  descriptorId: "kilogram:runtime-ipc-descriptor-id:cbor-v1\0",
  conversationId: "kilogram:conversation-label:v1\0",
  contactId: "kilogram:runtime-ipc-contact-id:cbor-v1\0",
  policyId: "kilogram:runtime-ipc-ticket-automation-policy-id:cbor-v1\0",
  snapshotId: "kilogram:runtime-ipc-ticket-automation-snapshot-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-ticket-automation-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-ticket-automation-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-ticket-automation-response-id:cbor-v1\0",
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
const someUint = (value) => array([uint(1), uint(value)]);

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

function configuration(input) {
  return array([
    uint(VERSION),
    bytes(fromHex(input.contact_id_hex, 32, "contact ID")),
    bytes(Buffer.from(input.conversation_label_utf8, "utf8")),
    bytes(fromHex(input.peer_account_id_hex, 32, "peer Account ID")),
    uint(input.enabled ? 1 : 0),
    bytes(Buffer.from(input.service_origin_utf8, "utf8")),
    uint(input.ttl_seconds),
    uint(input.refresh_before_seconds),
    uint(input.retry_base_seconds),
    uint(input.retry_max_seconds),
    uint(input.allowed_network_mask),
  ]);
}

function generate(input) {
  const signer = privateKey(fromHex(input.device_secret_key_hex, 32, "Device secret"));
  const device = publicBytes(signer);
  assert(device.equals(fromHex(input.device_id_hex, 32, "Device ID")),
    "Device secret does not match Device ID");
  const descriptor = fromHex(input.descriptor_signed_cbor_hex, undefined, "signed descriptor");
  const descriptorId = domainHash(DOMAINS.descriptorId, descriptor);
  assert(descriptorId.equals(fromHex(input.descriptor_id_hex, 32, "descriptor ID")),
    "descriptor ID changed");
  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const account = fromHex(input.account_id_hex, 32, "Account ID");
  const peerAccount = fromHex(input.peer_account_id_hex, 32, "peer Account ID");
  const conversationId = domainHash(
    DOMAINS.conversationId, Buffer.from(input.conversation_label_utf8, "utf8"),
  );
  const contactId = domainHash(DOMAINS.contactId, array([
    uint(VERSION), bytes(account), bytes(peerAccount), bytes(conversationId),
  ]));
  assert(contactId.equals(fromHex(input.contact_id_hex, 32, "contact ID")),
    "contact ID is not derived from the exact Account pair and conversation label");
  const config = configuration(input);
  const generation = 1;
  const policyPreimage = array([
    uint(VERSION), bytes(account), bytes(device), uint(generation), none(),
    uint(input.configured_at_unix_seconds), config,
  ]);
  const policyId = domainHash(DOMAINS.policyId, policyPreimage);

  const publish = array([
    uint(VERSION), uint(0), uint(4), someUint(205), someUint(205), someUint(805),
    uint(0), someUint(1), someUint(1105), uint(1),
  ]);
  const refresh = array([
    uint(VERSION), uint(1), uint(5), someUint(206), none(), someUint(211),
    uint(1), none(), none(), uint(2),
  ]);
  const policy = array([
    uint(VERSION), config, uint(generation), none(), bytes(policyId),
    uint(input.configured_at_unix_seconds), publish, refresh,
  ]);

  const configureRequestId = Buffer.alloc(32, 7);
  const configureRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(configureRequestId), bytes(bearer),
    uint(200), uint(230), array([uint(0), none(), config]),
  ]);
  const configureDigest = domainHash(DOMAINS.requestDigest, configureRequest);
  const configureResult = array([
    uint(0), uint(0), uint(input.observed_at_unix_seconds), uint(1), policy,
    uint(0), uint(0), uint(0),
  ]);
  const configureContent = array([
    uint(VERSION), bytes(descriptorId), bytes(configureRequestId), bytes(configureDigest),
    bytes(device), configureResult,
  ]);
  const configureResponse = sign(DOMAINS.responseSignature, configureContent, signer);

  const snapshotPreimage = array([
    uint(VERSION), bytes(account), bytes(device), uint(input.observed_at_unix_seconds),
    uint(1), array([policy]),
  ]);
  const snapshotId = domainHash(DOMAINS.snapshotId, snapshotPreimage);
  const statusRequestId = Buffer.alloc(32, 8);
  const statusRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(statusRequestId), bytes(bearer),
    uint(210), uint(230), array([uint(1)]),
  ]);
  const statusDigest = domainHash(DOMAINS.requestDigest, statusRequest);
  const statusResult = array([
    uint(1), uint(input.observed_at_unix_seconds), uint(1), bytes(snapshotId),
    array([policy]), uint(0), uint(0), uint(1),
  ]);
  const statusContent = array([
    uint(VERSION), bytes(descriptorId), bytes(statusRequestId), bytes(statusDigest),
    bytes(device), statusResult,
  ]);
  const statusResponse = sign(DOMAINS.responseSignature, statusContent, signer);

  const forbidden = [
    Buffer.from("ticket-plaintext"), Buffer.from("device-secret.key"),
    Buffer.from("publication-write-capability"),
  ];
  const ipc = Buffer.concat([
    configureRequest, configureResponse.cbor, statusRequest, statusResponse.cbor,
  ]);
  for (const value of forbidden) {
    assert(ipc.indexOf(value) === -1, `forbidden ${value.toString()} leaked into IPC`);
  }

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
assert(schema.milestone === "M1.1.48" && schema.status === "candidate-not-active",
  "runtime IPC schema is not the expected inactive M1.1.48 candidate");
assert(schema.domains.length === 78 && schema.objects.length === 38 &&
  schema.covered_commands.length === 34 && schema.covered_results.length === 33 &&
  schema.semantic_rules.length === 177,
  "runtime IPC schema registry counts changed");
assert(vectors.schema_version === 1 && vectors.milestone === "M1.1.47" &&
  vectors.status === "candidate-not-active", "ticket automation vector header changed");
for (const [name, domain] of Object.entries({
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
  "canonical runtime IPC ticket automation reference");

if (args.has("--self-test")) {
  const changed = structuredClone(vectors.input);
  changed.allowed_network_mask ^= 4;
  assert(generate(changed).configure.request.cbor_hex !== vectors.expected.configure.request.cbor_hex,
    "network-mask mutation did not change canonical request bytes");
  const changedOrigin = structuredClone(vectors.input);
  changedOrigin.service_origin_utf8 = "https://other.example/";
  assert(generate(changedOrigin).policy_id_hex !== vectors.expected.policy_id_hex,
    "service-origin mutation did not change policy identity");
  process.stdout.write("canonical_runtime_ipc_ticket_automation_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_runtime_ipc_ticket_automation_reference=verified\n");
