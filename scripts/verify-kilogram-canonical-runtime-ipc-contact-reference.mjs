import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  conversationId: "kilogram:conversation-label:v1\0",
  contactId: "kilogram:runtime-ipc-contact-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-contact-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-contact-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-contact-response-id:cbor-v1\0",
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
const some = (value) => array([uint(1), value]);

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function key(seedHex, label) {
  const seed = fromHex(seedHex, 32, label);
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
  const cbor = array([uint(VERSION), content, bytes(signature)]);
  return { content, preimage, signature, cbor };
}
function objectJson(value) {
  const id = domainHash(DOMAINS.requestDigest, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.requestDigest, value]).toString("hex"),
    object_id_hex: id.toString("hex"),
  };
}
function signedJson(value) {
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
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}

function request(core, input, requestId, command) {
  return array([
    uint(VERSION), bytes(fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID")),
    bytes(requestId), bytes(fromHex(core.input.bearer_token_hex, 32, "bearer token")),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds),
    command,
  ]);
}
function response(core, requestId, requestBytes, result, signer) {
  const requestDigest = domainHash(DOMAINS.requestDigest, requestBytes);
  const content = array([
    uint(VERSION), bytes(fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID")),
    bytes(requestId), bytes(requestDigest), bytes(signer.publicKey), result,
  ]);
  return signed(DOMAINS.responseSignature, content, signer);
}

function generate(core, input) {
  const signer = key(core.input.device_secret_key_hex, "Device seed");
  assert(signer.publicKey.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  const localAccount = fromHex(core.expected.account_id_hex, 32, "local Account ID");
  const peerAccount = fromHex(input.peer_account_id_hex, 32, "peer Account ID");
  const peerDevice = fromHex(input.peer_device_id_hex, 32, "peer Device ID");
  const conversationId = domainHash(DOMAINS.conversationId, Buffer.from(input.conversation_label));
  const contactIdContent = array([
    uint(VERSION), bytes(localAccount), bytes(peerAccount), bytes(conversationId),
  ]);
  const contactId = domainHash(DOMAINS.contactId, contactIdContent);

  const addRequestId = fromHex(input.add_request_id_hex, 32, "add request ID");
  const addCommand = array([
    uint(0), textBytes(input.conversation_label), bytes(peerAccount),
    textBytes(input.peer_descriptor_file),
  ]);
  const addRequest = request(core, input, addRequestId, addCommand);
  const addResult = array([
    uint(0), bytes(contactId), bytes(peerAccount), bytes(peerDevice),
    uint(input.endpoint_candidate_count), uint(input.endpoint_candidate_added ? 1 : 0),
    uint(input.inserted ? 1 : 0),
  ]);
  const addResponse = response(core, addRequestId, addRequest, addResult, signer);

  const statusRequestId = fromHex(input.status_request_id_hex, 32, "status request ID");
  const statusRequest = request(core, input, statusRequestId, array([uint(1)]));
  const status = array([
    uint(VERSION), bytes(localAccount), bytes(signer.publicKey), textBytes(input.state_directory),
    uint(input.authority_revision), uint(input.active_device_count),
    some(bytes(fromHex(input.receipt_id_hex, 32, "receipt ID"))),
    some(uint(input.receipt_generation)),
    bytes(fromHex(input.device_list_digest_hex, 32, "device-list digest")),
    textBytes(input.applied_device_list_file), textBytes(input.launch_device_list_file),
    uint(input.profile_convergence), uint(input.restart_recovery),
  ]);
  const statusResponse = response(
    core, statusRequestId, statusRequest, array([uint(1), status]), signer,
  );

  return {
    account_id_hex: localAccount.toString("hex"),
    device_id_hex: signer.publicKey.toString("hex"),
    peer_account_id_hex: peerAccount.toString("hex"),
    conversation_id_hex: conversationId.toString("hex"),
    contact_id_content_cbor_hex: contactIdContent.toString("hex"),
    contact_id_preimage_hex: Buffer.concat([DOMAINS.contactId, contactIdContent]).toString("hex"),
    contact_id_hex: contactId.toString("hex"),
    add_contact: { request: objectJson(addRequest), response: signedJson(addResponse) },
    directory_status: { request: objectJson(statusRequest), response: signedJson(statusResponse) },
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      add_command_result_pair: "add-contact/contact-added",
      status_command_result_pair: "own-device-directory-status/own-device-directory-status",
      response_binds_complete_request_digest: true,
      active_runtime_changed: false,
    },
  };
}

function parse(argv) {
  const result = { selfTest: false, emit: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (argument === "--emit") result.emit = true;
    else if (["--schema", "--vectors", "--core-vectors", "--output"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["core-vectors"], "--core-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  request_issued_at_unix_seconds: 1800002020,
  request_expires_at_unix_seconds: 1800002050,
  add_request_id_hex: "4142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f60",
  status_request_id_hex: "6162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f80",
  conversation_label: "m1130-contact",
  peer_account_id_hex: "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
  peer_device_id_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  peer_descriptor_file: "C:\\Kilogram\\peer-contact.ticket",
  endpoint_candidate_count: 2,
  endpoint_candidate_added: true,
  inserted: true,
  state_directory: "C:\\Kilogram\\state",
  authority_revision: 7,
  active_device_count: 2,
  receipt_id_hex: "909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeaf",
  receipt_generation: 3,
  device_list_digest_hex: "b0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecf",
  applied_device_list_file: "C:\\Kilogram\\state\\active-devices.bin",
  launch_device_list_file: "C:\\Kilogram\\profile\\launch-devices.bin",
  profile_convergence: 1,
  restart_recovery: 0,
};

const options = parse(process.argv.slice(2));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
if (options.emit) {
  const document = {
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependencies: ["CANONICAL-RUNTIME-IPC-VECTORS.json"],
    input: defaultInput,
    expected: generate(core, defaultInput),
  };
  const output = `${JSON.stringify(document, null, 2)}\n`;
  if (options.output) writeFileSync(options.output, output);
  else process.stdout.write(output);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.42" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC schema header changed");
assert(schema.limits.maximum_contact_response_bytes === 131072 &&
  schema.limits.maximum_account_devices === 32 &&
  schema.limits.maximum_endpoint_candidates_per_conversation === 4,
"runtime IPC contact bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v26-with-json-descriptor-v7" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active" &&
  JSON.stringify(vectors.dependencies) === JSON.stringify(["CANONICAL-RUNTIME-IPC-VECTORS.json"]),
"runtime IPC contact vector header changed");
equalJson(generate(core, vectors.input), vectors.expected,
  "independent canonical runtime IPC contact vector");

if (options.selfTest) {
  const changedLabel = structuredClone(vectors.input);
  changedLabel.conversation_label = "m1130-contact-tampered";
  const changed = generate(core, changedLabel);
  assert(changed.contact_id_hex !== vectors.expected.contact_id_hex,
    "changed contact label preserved canonical contact ID");
  assert(changed.add_contact.response.signature_hex !==
    vectors.expected.add_contact.response.signature_hex,
  "changed contact request preserved response signature");
  const changedReceipt = structuredClone(vectors.input);
  changedReceipt.receipt_generation += 1;
  assert(generate(core, changedReceipt).directory_status.response.signature_hex !==
    vectors.expected.directory_status.response.signature_hex,
  "changed directory receipt preserved response signature");
  process.stdout.write("changed_contact_identity=rejected\n");
  process.stdout.write("changed_directory_receipt=rejected\n");
  process.stdout.write("canonical_runtime_ipc_contact_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_contact_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("contact_enrollment=verified\n");
  process.stdout.write("device_directory_status=verified\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
