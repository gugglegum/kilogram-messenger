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
  artifactDigest: "kilogram:runtime-ipc-own-device-announcement-artifact-digest:cbor-v1\0",
  discoveryPolicyId: "kilogram:runtime-ipc-own-device-discovery-policy-id:cbor-v1\0",
  policyId: "kilogram:runtime-ipc-own-device-roster-policy-id:cbor-v1\0",
  snapshotId: "kilogram:runtime-ipc-own-device-roster-snapshot-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-own-device-roster-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-own-device-roster-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-own-device-roster-response-id:cbor-v1\0",
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
const someId = (value) => array([uint(1), bytes(value)]);

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
  return { content, preimage, signature, cbor: array([uint(VERSION), content, bytes(signature)]) };
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

function parentConfiguration(input) {
  return array([
    uint(VERSION), uint(input.enabled ? 1 : 0), bytes(Buffer.from(input.service_origin_utf8)),
    uint(input.publication_ttl_seconds), uint(input.refresh_before_seconds),
    uint(input.announcement_interval_seconds), uint(input.announcement_validity_seconds),
    uint(input.retry_base_seconds), uint(input.retry_max_seconds), uint(input.allowed_network_mask),
  ]);
}
function discoveryConfiguration(input, recipient, enabled) {
  return array([
    uint(VERSION), bytes(recipient), uint(enabled ? 1 : 0),
    bytes(Buffer.from(input.service_origin_utf8)), uint(input.publication_ttl_seconds),
    uint(input.refresh_before_seconds), uint(input.announcement_interval_seconds),
    uint(input.announcement_validity_seconds), uint(input.retry_base_seconds),
    uint(input.retry_max_seconds), uint(input.allowed_network_mask),
  ]);
}
function discoveryPolicyId(input, account, device, rosterDigest, configuration,
                           outbound, inbound) {
  return domainHash(DOMAINS.discoveryPolicyId, array([
    uint(VERSION), bytes(account), bytes(device), uint(input.authority_revision),
    bytes(rosterDigest), uint(1), none(), uint(input.configure_issued_at_unix_seconds),
    configuration, bytes(outbound), bytes(inbound),
  ]));
}
function discoveryStatus(input, account, device, rosterDigest, recipient, enabled,
                         outboundByte, synchronized) {
  const configuration = discoveryConfiguration(input, recipient, enabled);
  const outbound = Buffer.alloc(32, outboundByte);
  const inbound = Buffer.alloc(32, outboundByte + 1);
  const policyId = discoveryPolicyId(input, account, device, rosterDigest,
    configuration, outbound, inbound);
  const ticket = fromHex(input.recipient_ticket_hex, undefined, "recipient ticket");
  const ticketDigest = domainHash(DOMAINS.artifactDigest, ticket);
  assert(ticketDigest.toString("hex") === input.recipient_ticket_digest_hex,
    "recipient ticket digest changed");
  const status = array([
    uint(VERSION), configuration, uint(1), none(), bytes(policyId),
    uint(input.configure_issued_at_unix_seconds), bytes(outbound), bytes(inbound),
    synchronized ? someUint(3) : none(),
    synchronized ? array([uint(1), array([
      uint(VERSION), uint(4), uint(input.configure_issued_at_unix_seconds - 20),
      uint(input.configure_issued_at_unix_seconds + 880),
      array([uint(ticket.length), bytes(ticketDigest)]), bytes(Buffer.alloc(32, 0xe0)),
    ])]) : none(),
    synchronized ? someId(Buffer.alloc(32, 0xe1)) : none(),
    uint(synchronized ? 6 : (enabled ? 2 : 0)),
    synchronized ? someUint(input.configure_issued_at_unix_seconds - 4) : none(),
    synchronized ? someUint(input.configure_issued_at_unix_seconds - 4) : none(),
    synchronized ? someUint(input.configure_issued_at_unix_seconds + 296) : none(),
    uint(0), synchronized ? someId(Buffer.alloc(32, 0xe2)) : none(),
    synchronized ? array([uint(1), uint(1)]) : none(),
  ]);
  return { policyId, status };
}
function child(lifecycle, status) {
  return array([uint(VERSION), uint(lifecycle), bytes(status)]);
}
function responseJson(value, extra = {}) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    id_hex: domainHash(DOMAINS.responseId, value.cbor).toString("hex"),
    ...extra,
  };
}

function generate(input) {
  const signer = privateKey(fromHex(input.device_secret_key_hex, 32, "Device secret"));
  const device = publicBytes(signer);
  const localDevice = fromHex(input.local_device_id_hex, 32, "local Device ID");
  assert(device.equals(localDevice), "Device secret does not match local Device ID");
  const account = fromHex(input.account_id_hex, 32, "Account ID");
  const descriptorId = fromHex(input.descriptor_id_hex, 32, "descriptor ID");
  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const roster = fromHex(input.current_device_list_signed_cbor_hex, undefined, "current Device list");
  const rosterDigest = domainHash(DOMAINS.deviceListDigest, roster);
  assert(rosterDigest.toString("hex") === input.device_list_digest_hex,
    "current Device-list digest changed");
  const activeRecipient = fromHex(input.active_recipient_device_id_hex, 32, "active recipient");
  const retiredRecipient = fromHex(input.retired_recipient_device_id_hex, 32, "retired recipient");
  assert(Buffer.compare(retiredRecipient, activeRecipient) < 0,
    "fixture child ordering changed");
  const configuration = parentConfiguration(input);
  const policyPreimage = array([
    uint(VERSION), bytes(account), bytes(device), uint(input.authority_revision),
    bytes(rosterDigest), uint(input.parent_generation), none(),
    uint(input.configure_issued_at_unix_seconds), configuration,
  ]);
  const parentPolicyId = domainHash(DOMAINS.policyId, policyPreimage);
  assert(parentPolicyId.toString("hex") === input.parent_policy_id_hex,
    `parent policy ID changed: ${parentPolicyId.toString("hex")} != ${input.parent_policy_id_hex}`);
  const parentPolicy = array([
    uint(VERSION), configuration, uint(input.parent_generation), none(),
    bytes(parentPolicyId), uint(input.configure_issued_at_unix_seconds),
  ]);

  const retired = discoveryStatus(input, account, device, rosterDigest,
    retiredRecipient, false, 0xd0, false);
  const activeInitial = discoveryStatus(input, account, device, rosterDigest,
    activeRecipient, true, 0xd2, false);
  const activeStatus = discoveryStatus(input, account, device, rosterDigest,
    activeRecipient, true, 0xd2, true);
  assert(retired.policyId.toString("hex") === input.retired_discovery_policy_id_hex,
    "retired discovery policy ID changed");
  assert(activeInitial.policyId.toString("hex") === input.active_discovery_policy_id_hex,
    "active discovery policy ID changed");
  const initialChildren = array([child(1, retired.status), child(0, activeInitial.status)]);
  const statusChildren = array([child(1, retired.status), child(0, activeStatus.status)]);

  const snapshot = (children) => domainHash(DOMAINS.snapshotId, array([
    uint(VERSION), bytes(account), bytes(device), uint(input.authority_revision),
    bytes(rosterDigest), uint(1), parentPolicy, children,
  ]));
  const configureSnapshot = snapshot(initialChildren);
  const statusSnapshot = snapshot(statusChildren);

  const configureRequestId = fromHex(input.configure_request_id_hex, 32, "configure request ID");
  const configureRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(configureRequestId), bytes(bearer),
    uint(input.configure_issued_at_unix_seconds), uint(input.configure_expires_at_unix_seconds),
    array([uint(0), bytes(roster), none(), configuration]),
  ]);
  const configureDigest = domainHash(DOMAINS.requestDigest, configureRequest);
  const configureResult = array([
    uint(0), uint(0), uint(input.authority_revision), bytes(rosterDigest), uint(1),
    parentPolicy, bytes(configureSnapshot), initialChildren, uint(1), uint(1), uint(1),
    uint(0), uint(0), uint(0),
  ]);
  const configureContent = array([
    uint(VERSION), bytes(descriptorId), bytes(configureRequestId), bytes(configureDigest),
    bytes(device), configureResult,
  ]);
  const configureResponse = sign(DOMAINS.responseSignature, configureContent, signer);

  const statusRequestId = fromHex(input.status_request_id_hex, 32, "status request ID");
  const statusRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(statusRequestId), bytes(bearer),
    uint(input.status_issued_at_unix_seconds), uint(input.status_expires_at_unix_seconds),
    array([uint(1), bytes(roster)]),
  ]);
  const statusDigest = domainHash(DOMAINS.requestDigest, statusRequest);
  const statusResult = array([
    uint(1), uint(input.authority_revision), bytes(rosterDigest), uint(1), parentPolicy,
    bytes(statusSnapshot), statusChildren, uint(1), uint(1), uint(1), uint(0), uint(0),
  ]);
  const statusContent = array([
    uint(VERSION), bytes(descriptorId), bytes(statusRequestId), bytes(statusDigest),
    bytes(device), statusResult,
  ]);
  const statusResponse = sign(DOMAINS.responseSignature, statusContent, signer);

  const ticket = fromHex(input.recipient_ticket_hex, undefined, "recipient ticket");
  assert(Buffer.concat([configureRequest, configureResponse.cbor, statusRequest,
    statusResponse.cbor]).indexOf(ticket) === -1, "recipient ticket plaintext leaked into IPC");
  return {
    configure: {
      request: { cbor_hex: configureRequest.toString("hex"), digest_hex: configureDigest.toString("hex") },
      response: responseJson(configureResponse, {
        parent_policy_id_hex: parentPolicyId.toString("hex"),
        snapshot_id_hex: configureSnapshot.toString("hex"),
      }),
    },
    status: {
      request: { cbor_hex: statusRequest.toString("hex"), digest_hex: statusDigest.toString("hex") },
      response: responseJson(statusResponse, {
        snapshot_id_hex: statusSnapshot.toString("hex"), active_children: 1,
        retired_children: 1, max_parallel_workflows: 1,
      }),
    },
  };
}

function argumentsMap(argv) {
  const output = new Map();
  for (let index = 2; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === "--self-test") { output.set(key, true); continue; }
    const value = argv[index + 1]; assert(value !== undefined, `missing value after ${key}`);
    output.set(key, value); index += 1;
  }
  return output;
}

const args = argumentsMap(process.argv);
for (const required of ["--schema", "--vectors"]) assert(args.has(required), `missing ${required}`);
const schema = JSON.parse(readFileSync(args.get("--schema"), "utf8"));
const vectors = JSON.parse(readFileSync(args.get("--vectors"), "utf8"));
assert(schema.milestone === "M1.1.47" && schema.status === "candidate-not-active",
  "runtime IPC schema is not the expected inactive M1.1.47 candidate");
assert(schema.domains.length === 78 && schema.objects.length === 38 &&
  schema.covered_commands.length === 34 && schema.covered_results.length === 33 &&
  schema.semantic_rules.length === 175, "runtime IPC schema registry counts changed");
assert(vectors.schema_version === 1 && vectors.milestone === "M1.1.42" &&
  vectors.status === "candidate-not-active", "own-device roster vector header changed");
for (const [name, domain] of Object.entries({
  policy_id: DOMAINS.policyId, snapshot_id: DOMAINS.snapshotId,
  request_digest: DOMAINS.requestDigest, response_signature: DOMAINS.responseSignature,
  response_id: DOMAINS.responseId,
})) {
  assert(vectors.domains[name] === domain.subarray(0, domain.length - 1).toString("utf8"),
    `${name} domain changed`);
}
equalJson(generate(vectors.input), vectors.expected,
  "canonical runtime IPC own-device roster reference");

if (args.has("--self-test")) {
  const changed = structuredClone(vectors.input);
  changed.configure_request_id_hex = "dc".repeat(32);
  assert(generate(changed).configure.request.cbor_hex !== vectors.expected.configure.request.cbor_hex,
    "request-ID mutation did not change canonical bytes");
  process.stdout.write("canonical_runtime_ipc_own_device_roster_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_runtime_ipc_own_device_roster_reference=verified\n");
