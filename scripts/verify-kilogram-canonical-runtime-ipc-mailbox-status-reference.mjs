import {
  createPrivateKey, createPublicKey, sign as signEd25519, verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  descriptorSignature: "kilogram:runtime-ipc-descriptor-signature:cbor-v1\0",
  descriptorId: "kilogram:runtime-ipc-descriptor-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-mailbox-status-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-mailbox-status-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-mailbox-status-response-id:cbor-v1\0",
  snapshotId: "kilogram:runtime-ipc-mailbox-status-snapshot-id:cbor-v1\0",
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

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function key(seedHex, label) {
  const seed = fromHex(seedHex, 32, `${label} seed`);
  const privateKey = createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8",
  });
  const der = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(der.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI");
  return { privateKey, publicKey: der.subarray(SPKI_PREFIX.length) };
}
function publicFromBytes(raw) {
  return createPublicKey({ key: Buffer.concat([SPKI_PREFIX, raw]), format: "der", type: "spki" });
}
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function signed(domain, content, signer) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer.privateKey);
  assert(verifyEd25519(null, preimage, publicFromBytes(signer.publicKey), signature),
    "generated Ed25519 signature failed");
  return { content, preimage, signature, cbor: array([uint(VERSION), content, bytes(signature)]) };
}
function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value !== null && typeof value === "object")
    return Object.fromEntries(Object.keys(value).sort().map((name) => [name, stable(value[name])]));
  return value;
}
function equalJson(actual, expected, label) {
  if (JSON.stringify(stable(actual)) !== JSON.stringify(stable(expected))) fail(`${label} diverged`);
}
function requestJson(value) {
  const digest = domainHash(DOMAINS.requestDigest, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.requestDigest, value]).toString("hex"),
    object_id_hex: digest.toString("hex"),
  };
}
function responseJson(value) {
  const id = domainHash(DOMAINS.responseId, value.cbor);
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: DOMAINS.responseSignature.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.responseId, value.cbor]).toString("hex"),
    object_id_hex: id.toString("hex"),
  };
}

function verifyProvisioningDependency(provisioning, owner, recipient) {
  assert(owner.publicKey.toString("hex") === provisioning.expected.owner_device_id_hex,
    "owner Device dependency changed");
  assert(recipient.publicKey.toString("hex") === provisioning.expected.recipient_device_id_hex,
    "recipient Device dependency changed");
  for (const [name, signer] of [
    ["activation_update", owner], ["activation_acknowledgement", recipient],
  ]) {
    const value = provisioning.expected[name];
    const preimage = fromHex(value.signing_preimage_hex, undefined, `${name} preimage`);
    const signature = fromHex(value.signature_hex, 64, `${name} signature`);
    assert(verifyEd25519(null, preimage, publicFromBytes(signer.publicKey), signature),
      `${name} signature dependency changed`);
  }
}

function generate(provisioning, input) {
  const owner = key(provisioning.input.owner_secret_key_hex, "owner");
  const recipient = key(provisioning.input.recipient_secret_key_hex, "recipient");
  verifyProvisioningDependency(provisioning, owner, recipient);
  const accountId = fromHex(provisioning.input.owner_account_id_hex, 32, "owner Account ID");
  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const serverInstance = fromHex(input.server_instance_id_hex, 32, "server instance ID");
  const address = array([uint(0), bytes(Buffer.from([127, 0, 0, 1]))]);
  const descriptorContent = array([
    uint(VERSION), address, uint(input.port), bytes(bearer), bytes(accountId),
    bytes(owner.publicKey), bytes(serverInstance), uint(input.descriptor_issued_at_unix_seconds),
    uint(input.descriptor_expires_at_unix_seconds),
  ]);
  const descriptor = signed(DOMAINS.descriptorSignature, descriptorContent, owner);
  const descriptorId = domainHash(DOMAINS.descriptorId, descriptor.cbor);
  const update = fromHex(provisioning.expected.activation_update.signed_cbor_hex,
    undefined, "signed update");
  const acknowledgement = fromHex(
    provisioning.expected.activation_acknowledgement.signed_cbor_hex,
    undefined, "signed acknowledgement");
  const acknowledgementOption = array([uint(1), bytes(acknowledgement)]);
  const capabilityHead = array([uint(VERSION), bytes(update), acknowledgementOption]);
  const heads = array([capabilityHead]);
  const counts = array([
    uint(VERSION), uint(1), uint(1), uint(0), uint(1), uint(0), uint(1), uint(0),
  ]);
  const snapshotTranscript = array([
    uint(VERSION), bytes(accountId), bytes(owner.publicKey), counts, heads,
  ]);
  const snapshotId = domainHash(DOMAINS.snapshotId, snapshotTranscript);
  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const command = array([uint(0), array([uint(0)]), uint(input.page_limit)]);
  const request = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds), command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);
  const page = array([
    uint(VERSION), bytes(snapshotId), counts, heads, array([uint(0)]),
  ]);
  const result = array([uint(0), page]);
  const responseContent = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest),
    bytes(owner.publicKey), result,
  ]);
  const response = signed(DOMAINS.responseSignature, responseContent, owner);
  return {
    account_id_hex: accountId.toString("hex"),
    device_id_hex: owner.publicKey.toString("hex"),
    descriptor: {
      signed_cbor_hex: descriptor.cbor.toString("hex"),
      object_id_hex: descriptorId.toString("hex"),
    },
    snapshot_id_hex: snapshotId.toString("hex"),
    request: requestJson(request),
    response: responseJson(response),
    context: {
      valid_at_unix_seconds: input.valid_at_unix_seconds,
      head_count: 1,
      signed_updates_and_acknowledgements_only: true,
      read_write_secrets_in_payload: false,
      response_binds_complete_request_digest: true,
      active_runtime_changed: false,
    },
  };
}

function parse(argv) {
  const result = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--provisioning-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const required of ["schema", "vectors", "provisioning-vectors"])
    assert(result[required], `--${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const provisioning = JSON.parse(readFileSync(options["provisioning-vectors"], "utf8"));
assert(schema.milestone === "M1.1.48" && schema.status === "candidate-not-active",
  "mailbox status schema milestone or activation state changed");
equalJson(generate(provisioning, vectors.input), vectors.expected,
  "canonical runtime IPC mailbox status vectors");

if (options.selfTest) {
  const changed = structuredClone(vectors.input);
  changed.page_limit = 15;
  let rejected = false;
  try { equalJson(generate(provisioning, changed), vectors.expected, "changed page limit"); }
  catch { rejected = true; }
  assert(rejected, "changed page limit was unexpectedly accepted");
  const changedRequest = structuredClone(vectors.input);
  changedRequest.request_id_hex = `00${changedRequest.request_id_hex.slice(2)}`;
  rejected = false;
  try { equalJson(generate(provisioning, changedRequest), vectors.expected, "changed request ID"); }
  catch { rejected = true; }
  assert(rejected, "changed request ID was unexpectedly accepted");
  console.log("canonical_runtime_ipc_mailbox_status_reference_self_test=passed");
}
console.log("canonical_runtime_ipc_mailbox_status_reference=verified");
console.log("independent_blake3=verified");
