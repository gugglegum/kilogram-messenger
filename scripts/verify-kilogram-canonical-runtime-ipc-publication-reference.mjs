import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3DeriveKey, blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  channelId: "kilogram:ticket-publication-capability-channel:v1\0",
  peerScopeDigest: "kilogram:runtime-ipc-publication-peer-scope-digest:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-publication-request-digest:cbor-v1\0",
  rotationId: "kilogram:runtime-ipc-publication-rotation-id:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-publication-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-publication-response-id:cbor-v1\0",
}).map(([name, value]) => [name, Buffer.from(value)]));
const ROTATED_DERIVATION_CONTEXT =
  "kilogram ticket publication per-peer rotated write capability v1";

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
const optionDigest = (value) => value === undefined
  ? array([uint(0)])
  : array([uint(1), bytes(value)]);

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
function publicFromBytes(raw) {
  return createPublicKey({ key: Buffer.concat([SPKI_PREFIX, raw]), format: "der", type: "spki" });
}
function capability(deviceSecret, peerScope, epoch) {
  assert(BigInt(epoch) > 0n, "canonical rotated epoch must be positive");
  const seed = blake3DeriveKey(ROTATED_DERIVATION_CONTEXT,
    Buffer.concat([deviceSecret, peerScope, Buffer.from([0]), u64be(epoch)]));
  const key = privateKey(seed);
  const writeKey = publicBytes(key);
  const channelId = domainHash(DOMAINS.channelId, writeKey);
  const cbor = array([uint(VERSION), bytes(writeKey), bytes(channelId)]);
  return { key, writeKey, channelId, cbor };
}
function capabilityJson(value) {
  return {
    write_key_hex: value.writeKey.toString("hex"),
    channel_id_hex: value.channelId.toString("hex"),
    cbor_hex: value.cbor.toString("hex"),
  };
}
function signed(domain, content, signer) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer);
  const signerPublic = publicBytes(signer);
  assert(verifyEd25519(null, preimage, publicFromBytes(signerPublic), signature),
    "generated Ed25519 signature failed");
  return { content, preimage, signature, cbor: array([uint(VERSION), content, bytes(signature)]) };
}
function requestJson(value) {
  const id = domainHash(DOMAINS.requestDigest, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.requestDigest, value]).toString("hex"),
    object_id_hex: id.toString("hex"),
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

function generate(core, input) {
  const deviceSecret = fromHex(input.device_secret_hex, 32, "Device secret");
  const deviceSigner = privateKey(deviceSecret);
  const devicePublic = publicBytes(deviceSigner);
  assert(devicePublic.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  const descriptorId = fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID");
  const bearer = fromHex(core.input.bearer_token_hex, 32, "bearer token");
  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const peerScope = fromHex(input.peer_scope_hex, undefined, "peer scope");
  assert(peerScope.length > 0 && peerScope.length <= 4096, "peer scope is outside bounds");
  const contactId = fromHex(input.contact_id_hex, 32, "contact ID");
  const peerAccount = fromHex(input.peer_account_id_hex, 32, "peer Account ID");
  const latestPublicationId = fromHex(input.latest_publication_id_hex, 32,
    "latest publication ID");
  assert(BigInt(input.next_channel_epoch) === BigInt(input.current_channel_epoch) + 1n,
    "next publication channel epoch must advance exactly once");
  const peerScopeDigest = domainHash(DOMAINS.peerScopeDigest, peerScope);
  const previousCapability = capability(deviceSecret, peerScope, input.current_channel_epoch);
  const nextCapability = capability(deviceSecret, peerScope, input.next_channel_epoch);
  const currentState = array([
    uint(VERSION), bytes(contactId), bytes(peerAccount), bytes(peerScopeDigest),
    uint(input.current_channel_epoch), bytes(previousCapability.cbor),
    optionDigest(latestPublicationId), uint(input.latest_publication_generation),
  ]);
  const command = array([uint(0), currentState, uint(input.next_channel_epoch)]);
  const request = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds),
    command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);
  const rotationContext = array([
    uint(VERSION), bytes(requestDigest), bytes(nextCapability.cbor), uint(input.next_channel_epoch),
  ]);
  const rotationId = domainHash(DOMAINS.rotationId, rotationContext);
  const rotation = array([
    uint(VERSION), uint(input.outcome === "committed" ? 0 : 1),
    bytes(contactId), bytes(peerAccount), bytes(peerScopeDigest),
    bytes(previousCapability.cbor), uint(input.current_channel_epoch),
    bytes(nextCapability.cbor), uint(input.next_channel_epoch), bytes(rotationId),
    optionDigest(latestPublicationId), uint(input.latest_publication_generation),
    uint(0), uint(0), uint(0),
  ]);
  const result = array([uint(0), bytes(rotation)]);
  const content = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest),
    bytes(devicePublic), result,
  ]);
  const response = signed(DOMAINS.responseSignature, content, deviceSigner);
  return {
    account_id_hex: core.expected.account_id_hex,
    device_id_hex: devicePublic.toString("hex"),
    peer_scope_digest_hex: peerScopeDigest.toString("hex"),
    previous_capability: capabilityJson(previousCapability),
    capability: capabilityJson(nextCapability),
    rotation_id_hex: rotationId.toString("hex"),
    rotation_cbor_hex: rotation.toString("hex"),
    request: requestJson(request),
    response: responseJson(response),
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      successor_derived_inside_runtime: true,
      local_device_secret_in_payload: false,
      network_publication_executed: false,
      peer_delivery_executed: false,
      active_runtime_changed: false,
    },
  };
}

function parse(argv) {
  const result = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--core-vectors", "--contact-vectors",
      "--ticket-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const required of ["schema", "vectors", "core-vectors", "contact-vectors",
    "ticket-vectors"]) assert(result[required], `--${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
const contact = JSON.parse(readFileSync(options["contact-vectors"], "utf8"));
const ticket = JSON.parse(readFileSync(options["ticket-vectors"], "utf8"));
assert(schema.milestone === "M1.1.39" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC publication schema header changed");
assert(schema.limits.maximum_publication_request_bytes === 65536 &&
  schema.limits.maximum_publication_response_bytes === 65536,
"runtime IPC publication bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v26-with-json-descriptor-v7" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active" &&
  JSON.stringify(vectors.dependencies) === JSON.stringify([
    "CANONICAL-RUNTIME-IPC-VECTORS.json",
    "CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json",
    "CANONICAL-TICKET-PUBLICATION-VECTORS.json",
  ]), "runtime IPC publication vector header changed");
assert(vectors.input.peer_account_id_hex === contact.input.peer_account_id_hex,
  "contact dependency peer Account changed");
assert(ticket.expected.domains.rotated_derivation_context === ROTATED_DERIVATION_CONTEXT,
  "ticket publication derivation context changed");
equalJson(generate(core, vectors.input), vectors.expected,
  "independent canonical runtime IPC publication vector");

if (options.selfTest) {
  const changedScope = structuredClone(vectors.input);
  changedScope.peer_scope_hex = `ff${changedScope.peer_scope_hex.slice(2)}`;
  assert(generate(core, changedScope).request.object_id_hex !==
    vectors.expected.request.object_id_hex, "changed peer scope preserved request digest");
  const changedEpoch = structuredClone(vectors.input);
  changedEpoch.current_channel_epoch += 1;
  changedEpoch.next_channel_epoch += 1;
  assert(generate(core, changedEpoch).capability.write_key_hex !==
    vectors.expected.capability.write_key_hex, "changed epoch preserved successor capability");
  assert(generate(core, changedEpoch).rotation_id_hex !== vectors.expected.rotation_id_hex,
    "changed epoch preserved rotation ID");
  process.stdout.write("changed_publication_peer_scope=rejected\n");
  process.stdout.write("changed_publication_epoch=rejected\n");
  process.stdout.write("canonical_runtime_ipc_publication_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_publication_reference=verified\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("independent_blake3_hash_and_derive_key=verified\n");
  process.stdout.write("successor_derived_inside_runtime=true\n");
  process.stdout.write("local_device_secret_in_payload=false\n");
  process.stdout.write("network_executed=false\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
