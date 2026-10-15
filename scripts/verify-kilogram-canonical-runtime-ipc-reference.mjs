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
  descriptorSignature: "kilogram:runtime-ipc-descriptor-signature:cbor-v1\0",
  descriptorId: "kilogram:runtime-ipc-descriptor-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-response-id:cbor-v1\0",
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
function signedJson(value, idDomain) {
  const id = domainHash(idDomain, value.cbor);
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: value.preimage.subarray(0, value.preimage.length - value.content.length)
      .toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    id_preimage_hex: Buffer.concat([idDomain, value.cbor]).toString("hex"),
    object_id_hex: id.toString("hex"),
  };
}
function objectJson(value, idDomain) {
  const id = domainHash(idDomain, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: idDomain.toString("hex"),
    id_preimage_hex: Buffer.concat([idDomain, value]).toString("hex"),
    object_id_hex: id.toString("hex"),
  };
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}

function generate(authority, input) {
  const signer = key(input.device_secret_key_hex, "Device seed");
  const accountId = fromHex(authority.expected.account_id_hex, 32, "Account ID");
  assert(signer.publicKey.toString("hex") === authority.expected.device_id_hex,
    "Device dependency changed");
  const address = fromHex(input.address_hex, 4, "IPv4 loopback address");
  assert(address.equals(Buffer.from([127, 0, 0, 1])), "fixture address is not IPv4 loopback");
  const token = fromHex(input.bearer_token_hex, 32, "bearer token");
  const instanceId = fromHex(input.server_instance_id_hex, 32, "server instance ID");
  const descriptorContent = array([
    uint(VERSION), array([uint(0), bytes(address)]), uint(input.port), bytes(token),
    bytes(accountId), bytes(signer.publicKey), bytes(instanceId),
    uint(input.descriptor_issued_at_unix_seconds),
    uint(input.descriptor_expires_at_unix_seconds),
  ]);
  const descriptor = signed(DOMAINS.descriptorSignature, descriptorContent, signer);
  const descriptorId = domainHash(DOMAINS.descriptorId, descriptor.cbor);

  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const command = array([
    uint(1), uint(input.after_revision), uint(input.timeout_milliseconds),
  ]);
  const request = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(token),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds),
    command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);
  const result = array([uint(1), uint(input.response_revision), uint(input.response_changed ? 1 : 0)]);
  const responseContent = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest),
    bytes(signer.publicKey), result,
  ]);
  const response = signed(DOMAINS.responseSignature, responseContent, signer);

  return {
    account_id_hex: accountId.toString("hex"),
    device_id_hex: signer.publicKey.toString("hex"),
    descriptor: signedJson(descriptor, DOMAINS.descriptorId),
    request: objectJson(request, DOMAINS.requestDigest),
    response: signedJson(response, DOMAINS.responseId),
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      descriptor_matches_request: true,
      request_digest_matches_response: true,
      responder_matches_descriptor: true,
      command_result_pair: "wait-for-change/change-state",
    },
  };
}

function parse(argv) {
  const result = { selfTest: false, emit: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (argument === "--emit") result.emit = true;
    else if (["--schema", "--vectors", "--authority-vectors", "--output"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["authority-vectors"], "--authority-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  device_secret_key_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  address_hex: "7f000001",
  port: 43125,
  bearer_token_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  server_instance_id_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  descriptor_issued_at_unix_seconds: 1800002000,
  descriptor_expires_at_unix_seconds: 1800005600,
  request_id_hex: "e0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
  request_issued_at_unix_seconds: 1800002005,
  request_expires_at_unix_seconds: 1800002035,
  after_revision: 41,
  timeout_milliseconds: 30000,
  response_revision: 42,
  response_changed: true,
};

const options = parse(process.argv.slice(2));
const authority = JSON.parse(readFileSync(options["authority-vectors"], "utf8"));
if (options.emit) {
  const document = {
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependencies: ["CANONICAL-AUTHORITY-VECTORS.json"],
    input: defaultInput,
    expected: generate(authority, defaultInput),
  };
  const output = `${JSON.stringify(document, null, 2)}\n`;
  if (options.output) writeFileSync(options.output, output);
  else process.stdout.write(output);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.34" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC schema header changed");
assert(schema.cddl.path === "CANONICAL-RUNTIME-IPC.cddl" &&
  schema.cddl.root_rule === "kilogram-runtime-ipc-v1", "runtime IPC CDDL binding changed");
assert(schema.activation.current_runtime_encoding === "postcard-v26-with-json-descriptor-v7" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "runtime IPC vector header changed");
equalJson(generate(authority, vectors.input), vectors.expected,
  "independent canonical runtime IPC vector");

if (options.selfTest) {
  const changedRequest = structuredClone(vectors.input);
  changedRequest.request_id_hex = `00${changedRequest.request_id_hex.slice(2)}`;
  const changed = generate(authority, changedRequest);
  assert(changed.request.object_id_hex !== vectors.expected.request.object_id_hex,
    "changed request preserved request digest");
  assert(changed.response.signature_hex !== vectors.expected.response.signature_hex,
    "changed request preserved response signature");
  const tampered = structuredClone(vectors.expected);
  tampered.response.signature_hex = `00${tampered.response.signature_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(authority, vectors.input), tampered, "tampered runtime IPC vector"); }
  catch { rejected = true; }
  assert(rejected, "tampered response signature vector was accepted");
  process.stdout.write("changed_request_binding=rejected\n");
  process.stdout.write("tampered_response_signature=rejected\n");
  process.stdout.write("canonical_runtime_ipc_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519-and-javascript-blake3\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("signed_exact_response_binding=verified\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
