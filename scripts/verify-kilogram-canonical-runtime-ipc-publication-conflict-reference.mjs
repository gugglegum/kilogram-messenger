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
  descriptorSignature: "kilogram:runtime-ipc-descriptor-signature:cbor-v1\0",
  descriptorId: "kilogram:runtime-ipc-descriptor-id:cbor-v1\0",
  artifactDigest:
    "kilogram:runtime-ipc-publication-conflict-artifact-digest:cbor-v1\0",
  requestDigest:
    "kilogram:runtime-ipc-publication-conflict-request-digest:cbor-v1\0",
  responseSignature:
    "kilogram:runtime-ipc-publication-conflict-response-signature:cbor-v1\0",
  responseId:
    "kilogram:runtime-ipc-publication-conflict-response-id:cbor-v1\0",
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
const someBytes = (value) => array([uint(1), bytes(value)]);

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
function contains(haystack, needle) {
  return needle.length > 0 && haystack.indexOf(needle) !== -1;
}

function generate(input, conflict) {
  const signer = privateKey(fromHex(input.device_secret_hex, 32, "Device secret"));
  const device = publicBytes(signer);
  const account = fromHex(conflict.expected.public_keys.local_account_id_hex, 32,
    "local Account ID");
  assert(device.toString("hex") === conflict.expected.public_keys.detector_device_id_hex,
    "detector Device dependency changed");
  const peerAccount = fromHex(conflict.expected.public_keys.publisher_account_id_hex, 32,
    "peer Account ID");
  const peerDevice = fromHex(conflict.expected.public_keys.publisher_device_id_hex, 32,
    "peer Device ID");
  const proofId = fromHex(conflict.expected.conflict_proof.proof_id_hex, 32, "conflict proof ID");
  const evidenceId = fromHex(conflict.expected.conflict_proof.evidence_id_hex, 32,
    "conflict evidence ID");
  const oldChannel = fromHex(conflict.expected.public_keys.old_channel_id_hex, 32,
    "old publication channel ID");
  const requestClaim = fromHex(conflict.expected.request_claim.cbor_hex, undefined,
    "request claim");
  const responseClaim = fromHex(conflict.expected.response_claim.cbor_hex, undefined,
    "response claim");
  const endpointTicket = fromHex(input.endpoint_ticket_utf8_hex, undefined, "endpoint ticket");
  const replacementTicket = fromHex(input.replacement_ticket_utf8_hex, undefined,
    "replacement ticket");
  const requestArtifact = fromHex(input.request_artifact_fixture_hex, undefined,
    "request artifact");
  const responseArtifact = fromHex(input.response_artifact_fixture_hex, undefined,
    "response artifact");
  assert(requestArtifact.toString("hex") === conflict.expected.resolution_request.signed_cbor_hex,
    "request artifact dependency changed");
  assert(responseArtifact.toString("hex") === conflict.expected.resolution_response.cbor_hex,
    "response artifact dependency changed");

  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const serverInstance = fromHex(input.server_instance_id_hex, 32, "server instance ID");
  const ipv4 = fromHex(input.descriptor_ipv4_hex, 4, "descriptor IPv4");
  const descriptorContent = array([
    uint(VERSION), array([uint(0), bytes(ipv4)]), uint(input.descriptor_port), bytes(bearer),
    bytes(account), bytes(device), bytes(serverInstance),
    uint(input.descriptor_issued_at_unix_seconds), uint(input.descriptor_expires_at_unix_seconds),
  ]);
  const descriptor = sign(DOMAINS.descriptorSignature, descriptorContent, signer);
  const descriptorId = domainHash(DOMAINS.descriptorId, descriptor.cbor);
  const deviceListDigest = fromHex(input.device_list_digest_hex, 32, "Device-list digest");
  const endpointDigest = domainHash(DOMAINS.artifactDigest, endpointTicket);
  const replacementDigest = domainHash(DOMAINS.artifactDigest, replacementTicket);
  const requestArtifactDigest = domainHash(DOMAINS.artifactDigest, requestArtifact);
  const responseArtifactDigest = domainHash(DOMAINS.artifactDigest, responseArtifact);
  const authorityRevision = conflict.input.authority_revision;
  const oldEpoch = conflict.input.old_channel_epoch;

  const createCurrent = array([
    uint(VERSION), uint(authorityRevision), bytes(deviceListDigest), bytes(proofId),
    bytes(evidenceId), bytes(peerAccount), bytes(peerDevice), bytes(oldChannel), uint(oldEpoch),
    uint(endpointTicket.length), bytes(endpointDigest), none(),
  ]);
  const createRequestId = fromHex(input.create_request_id_hex, 32, "create request ID");
  const createCommand = array([
    uint(0), bytes(createCurrent), uint(replacementTicket.length), bytes(replacementDigest),
  ]);
  const createRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(createRequestId), bytes(bearer),
    uint(input.create_issued_at_unix_seconds), uint(input.create_expires_at_unix_seconds),
    createCommand,
  ]);
  const createRequestDigest = domainHash(DOMAINS.requestDigest, createRequest);
  const createResult = array([
    uint(VERSION), uint(0), bytes(requestClaim), bytes(proofId), bytes(device),
    uint(input.create_issued_at_unix_seconds), uint(requestArtifact.length),
    bytes(requestArtifactDigest), uint(replacementTicket.length), bytes(replacementDigest),
    uint(0), uint(0), uint(0), uint(0),
  ]);
  const createResponseContent = array([
    uint(VERSION), bytes(descriptorId), bytes(createRequestId), bytes(createRequestDigest),
    bytes(device), array([uint(0), bytes(createResult)]),
  ]);
  const createResponse = sign(DOMAINS.responseSignature, createResponseContent, signer);

  const pending = array([
    uint(VERSION), bytes(requestClaim), bytes(device), uint(input.create_issued_at_unix_seconds),
    uint(requestArtifact.length), bytes(requestArtifactDigest),
  ]);
  const applyCurrent = array([
    uint(VERSION), uint(authorityRevision), bytes(deviceListDigest), bytes(proofId),
    bytes(evidenceId), bytes(peerAccount), bytes(peerDevice), bytes(oldChannel), uint(oldEpoch),
    uint(endpointTicket.length), bytes(endpointDigest), someBytes(pending),
  ]);
  const applyRequestId = fromHex(input.apply_request_id_hex, 32, "apply request ID");
  const applyCommand = array([
    uint(1), bytes(applyCurrent), uint(responseArtifact.length), bytes(responseArtifactDigest),
  ]);
  const applyRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(applyRequestId), bytes(bearer),
    uint(input.apply_issued_at_unix_seconds), uint(input.apply_expires_at_unix_seconds),
    applyCommand,
  ]);
  const applyRequestDigest = domainHash(DOMAINS.requestDigest, applyRequest);
  const applyResult = array([
    uint(VERSION), uint(0), bytes(responseClaim), bytes(proofId),
    uint(conflict.input.authorized_at_unix_seconds), uint(responseArtifact.length),
    bytes(responseArtifactDigest), uint(1), uint(0), uint(0), uint(0), uint(0), uint(0), uint(0),
  ]);
  const applyResponseContent = array([
    uint(VERSION), bytes(descriptorId), bytes(applyRequestId), bytes(applyRequestDigest),
    bytes(device), array([uint(1), bytes(applyResult)]),
  ]);
  const applyResponse = sign(DOMAINS.responseSignature, applyResponseContent, signer);

  const ipcBytes = Buffer.concat([createRequest, createResponse.cbor, applyRequest, applyResponse.cbor]);
  for (const protectedValue of [endpointTicket, replacementTicket, requestArtifact, responseArtifact]) {
    assert(!contains(ipcBytes, protectedValue), "protected publication-conflict artifact leaked into IPC");
  }

  return {
    account_id_hex: account.toString("hex"),
    apply: {
      current_cbor_hex: applyCurrent.toString("hex"),
      request: requestJson(applyRequest),
      response: responseJson(applyResponse),
      result_cbor_hex: applyResult.toString("hex"),
    },
    artifact_digest_domain_hex: DOMAINS.artifactDigest.toString("hex"),
    context: {
      active_runtime_changed: false,
      artifact_broker_activated: false,
      conflict_proof_retained_after_resolution: true,
      local_path_in_ipc: false,
      network_io_executed: false,
      publication_write_key_in_ipc: false,
      replacement_ticket_plaintext_in_ipc: false,
      request_artifact_staged_before_success: true,
      response_verified_before_atomic_install: true,
      root_resolution_plaintext_in_ipc: false,
      root_secret_in_ipc: false,
      sibling_propagation_still_pending: true,
    },
    create: {
      current_cbor_hex: createCurrent.toString("hex"),
      request: requestJson(createRequest),
      response: responseJson(createResponse),
      result_cbor_hex: createResult.toString("hex"),
    },
    descriptor_id_hex: descriptorId.toString("hex"),
    descriptor_signed_cbor_hex: descriptor.cbor.toString("hex"),
    device_id_hex: device.toString("hex"),
    endpoint_ticket_artifact_digest_hex: endpointDigest.toString("hex"),
    replacement_ticket_artifact_digest_hex: replacementDigest.toString("hex"),
    request_artifact_digest_hex: requestArtifactDigest.toString("hex"),
    response_artifact_digest_hex: responseArtifactDigest.toString("hex"),
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
for (const required of ["--schema", "--vectors", "--conflict-vectors"]) {
  assert(args.has(required), `missing ${required}`);
}
const schema = JSON.parse(readFileSync(args.get("--schema"), "utf8"));
const vectors = JSON.parse(readFileSync(args.get("--vectors"), "utf8"));
const conflict = JSON.parse(readFileSync(args.get("--conflict-vectors"), "utf8"));
assert(schema.milestone === "M1.1.45" && schema.status === "candidate-not-active",
  "runtime IPC schema is not the expected inactive M1.1.45 candidate");
assert(vectors.schema_version === 1 && vectors.profile === "kilogram-deterministic-cbor-v1" &&
  vectors.activation_status === "not-active", "vector header changed");
const actual = generate(vectors.input, conflict);
assert(actual.descriptor_signed_cbor_hex === vectors.expected.descriptor_signed_cbor_hex,
  `runtime descriptor diverged: actual=${actual.descriptor_signed_cbor_hex} expected=${vectors.expected.descriptor_signed_cbor_hex}`);
equalJson(actual, vectors.expected,
  "canonical runtime IPC publication-conflict reference");

if (args.has("--self-test")) {
  const changed = structuredClone(vectors.input);
  changed.create_request_id_hex = `00${changed.create_request_id_hex.slice(2)}`;
  assert(generate(changed, conflict).create.request.cbor_hex !==
    vectors.expected.create.request.cbor_hex, "request-ID mutation did not change canonical bytes");
  process.stdout.write("canonical_runtime_ipc_publication_conflict_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_runtime_ipc_publication_conflict_reference=verified\n");
