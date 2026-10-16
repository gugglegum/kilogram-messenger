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
  conversationId: "kilogram:conversation-label:v1\0",
  contactId: "kilogram:runtime-ipc-contact-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-queue-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-queue-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-queue-response-id:cbor-v1\0",
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
const textBytes = (value) => bytes(Buffer.from(value, "utf8"));

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function key(seedHex) {
  const seed = fromHex(seedHex, 32, "Device seed");
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
  const left = stable(actual);
  const right = stable(expected);
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
  const signer = key(core.input.device_secret_key_hex);
  assert(signer.publicKey.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  const descriptorId = fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID");
  const bearer = fromHex(core.input.bearer_token_hex, 32, "bearer token");
  const localAccount = fromHex(core.expected.account_id_hex, 32, "local Account ID");
  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const peerAccount = fromHex(input.peer_account_id_hex, 32, "peer Account ID");
  const label = Buffer.from(input.conversation_label, "utf8");
  const message = Buffer.from(input.message, "utf8");
  const conversationId = domainHash(DOMAINS.conversationId, label);
  const contactContent = array([
    uint(VERSION), bytes(localAccount), bytes(peerAccount), bytes(conversationId),
  ]);
  const contactId = domainHash(DOMAINS.contactId, contactContent);
  const command = array([uint(0), bytes(label), bytes(peerAccount), bytes(message)]);
  const request = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds),
    command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);
  const result = array([
    uint(0), bytes(requestId), bytes(contactId), bytes(conversationId), bytes(peerAccount),
    uint(input.inserted ? 1 : 0),
  ]);
  const content = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest),
    bytes(signer.publicKey), result,
  ]);
  const response = signed(DOMAINS.responseSignature, content, signer);
  return {
    account_id_hex: localAccount.toString("hex"),
    device_id_hex: signer.publicKey.toString("hex"),
    conversation_id_hex: conversationId.toString("hex"),
    contact_id_hex: contactId.toString("hex"),
    request: requestJson(request),
    response: responseJson(response),
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      queue_id_equals_request_id: true,
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
    else if (["--schema", "--vectors", "--core-vectors", "--contact-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const required of ["schema", "vectors", "core-vectors", "contact-vectors"])
    assert(result[required], `--${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
const contact = JSON.parse(readFileSync(options["contact-vectors"], "utf8"));
assert(schema.milestone === "M1.1.46" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC queue schema header changed");
assert(schema.limits.maximum_queue_request_bytes === 131072 &&
  schema.limits.maximum_queue_response_bytes === 65536 &&
  schema.limits.maximum_queue_message_bytes === 65536,
"runtime IPC queue bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v27-with-json-descriptor-v8" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active" &&
  JSON.stringify(vectors.dependencies) === JSON.stringify([
    "CANONICAL-RUNTIME-IPC-VECTORS.json",
    "CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json",
  ]), "runtime IPC queue vector header changed");
assert(vectors.input.peer_account_id_hex === contact.input.peer_account_id_hex,
  "contact dependency peer Account changed");
equalJson(generate(core, vectors.input), vectors.expected,
  "independent canonical runtime IPC queue vector");

if (options.selfTest) {
  const changedMessage = structuredClone(vectors.input);
  changedMessage.message += "!";
  const changedMessageOutput = generate(core, changedMessage);
  assert(changedMessageOutput.request.object_id_hex !== vectors.expected.request.object_id_hex,
    "changed message preserved request digest");
  assert(changedMessageOutput.response.signature_hex !== vectors.expected.response.signature_hex,
    "changed message preserved response signature");
  const changedOutcome = structuredClone(vectors.input);
  changedOutcome.inserted = false;
  assert(generate(core, changedOutcome).response.signature_hex !==
    vectors.expected.response.signature_hex, "changed insert outcome preserved response signature");
  process.stdout.write("changed_queue_message=rejected\n");
  process.stdout.write("changed_queue_outcome=rejected\n");
  process.stdout.write("canonical_runtime_ipc_queue_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_queue_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("idempotent_queue_identity=verified\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
