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
  requestDigest: "kilogram:runtime-ipc-read-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-read-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-read-response-id:cbor-v1\0",
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
const none = () => array([uint(0)]);
const some = (value) => array([uint(1), value]);
const optionalUint = (value) => value === null ? none() : some(uint(value));
const optionalText = (value) => value === null ? none() : some(textBytes(value));

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
function objectJson(value) {
  const id = domainHash(DOMAINS.requestDigest, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.requestDigest, value]).toString("hex"),
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
function endpointCandidate(value) {
  return array([
    bytes(fromHex(value.peer_device_id_hex, 32, "peer Device ID")),
    uint(value.primary ? 1 : 0), uint(value.route_policy), textBytes(value.descriptor_path),
    uint(value.state), optionalUint(value.authority_revision),
    optionalText(value.publication_channel_id),
    optionalUint(value.observed_publication_generation),
    optionalUint(value.observed_at_unix_seconds),
    optionalUint(value.publication_conflict_generation),
    optionalText(value.publication_conflict_proof_id),
    optionalUint(value.publication_conflict_detected_at_unix_seconds),
    textBytes(value.detail),
  ]);
}
function generate(core, input) {
  const signer = key(core.input.device_secret_key_hex, "Device seed");
  assert(signer.publicKey.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  const descriptorId = fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID");
  const peerAccount = fromHex(input.peer_account_id_hex, 32, "peer Account ID");
  const primaryDevice = fromHex(input.primary_peer_device_id_hex, 32, "primary peer Device ID");
  const conversationLabel = Buffer.from(input.conversation_label, "utf8");
  const conversationId = domainHash(DOMAINS.conversationId, conversationLabel);

  const listRequestId = fromHex(input.list_request_id_hex, 32, "list request ID");
  const listCommand = array([uint(0), none(), uint(input.conversation_page_limit)]);
  const listRequest = request(core, input, listRequestId, listCommand);
  const candidates = input.endpoint_candidates.map(endpointCandidate);
  const preview = array([
    bytes(fromHex(input.preview_event_id_hex, 32, "preview event ID")), bytes(peerAccount),
    textBytes(input.preview_body), uint(input.preview_truncated ? 1 : 0),
  ]);
  const summary = array([
    textBytes(input.contact_id), textBytes(input.conversation_label), bytes(conversationId),
    bytes(peerAccount), bytes(primaryDevice), uint(candidates.length),
    uint(input.usable_endpoint_count), uint(input.stale_endpoint_count),
    uint(input.quarantined_endpoint_count), array(candidates), uint(input.summary_route_policy),
    uint(input.total_messages), some(preview),
  ]);
  const listResult = array([uint(0), array([summary]), none()]);
  const listResponse = response(core, listRequestId, listRequest, listResult, signer);

  const historyRequestId = fromHex(input.history_request_id_hex, 32, "history request ID");
  const historyCommand = array([
    uint(1), bytes(conversationLabel), none(), uint(input.history_page_limit),
  ]);
  const historyRequest = request(core, input, historyRequestId, historyCommand);
  const snapshotId = fromHex(input.history_snapshot_id_hex, 32, "history snapshot ID");
  const messages = input.history_messages.map((message) => array([
    bytes(fromHex(message.event_id_hex, 32, "history event ID")),
    bytes(fromHex(message.author_account_id_hex, 32, "history author Account ID")),
    bytes(fromHex(message.author_device_id_hex, 32, "history author Device ID")),
    uint(message.author_sequence), textBytes(message.body),
  ]));
  const nextCursor = some(array([bytes(snapshotId), uint(input.history_next_before_index)]));
  const historyPage = array([
    bytes(conversationId), bytes(snapshotId), uint(input.total_messages), array(messages), nextCursor,
  ]);
  const historyResult = array([uint(1), historyPage]);
  const historyResponse = response(core, historyRequestId, historyRequest, historyResult, signer);

  return {
    account_id_hex: core.expected.account_id_hex,
    device_id_hex: signer.publicKey.toString("hex"),
    descriptor_id_hex: descriptorId.toString("hex"),
    conversation_id_hex: conversationId.toString("hex"),
    conversation_list: {
      request: objectJson(listRequest),
      response: signedJson(listResponse),
    },
    history_page: {
      request: objectJson(historyRequest),
      response: signedJson(historyResponse),
    },
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      list_command_result_pair: "conversation-list-page/conversation-list-page",
      history_command_result_pair: "history-page/history-page",
      response_binds_complete_request_digest: true,
      history_snapshot_cursor_bound: true,
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
  request_issued_at_unix_seconds: 1800002010,
  request_expires_at_unix_seconds: 1800002040,
  list_request_id_hex: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
  history_request_id_hex: "2122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40",
  conversation_page_limit: 2,
  history_page_limit: 2,
  contact_id: "contact-m1129-read-model",
  conversation_label: "m1129-read-model",
  peer_account_id_hex: "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
  primary_peer_device_id_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  summary_route_policy: 0,
  usable_endpoint_count: 1,
  stale_endpoint_count: 0,
  quarantined_endpoint_count: 1,
  total_messages: 3,
  preview_event_id_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  preview_body: "latest preview",
  preview_truncated: false,
  endpoint_candidates: [
    {
      peer_device_id_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
      primary: true,
      route_policy: 0,
      descriptor_path: "C:\\Kilogram\\peer-primary.ticket",
      state: 0,
      authority_revision: 7,
      publication_channel_id: "channel-m1129",
      observed_publication_generation: 3,
      observed_at_unix_seconds: 1800001990,
      publication_conflict_generation: null,
      publication_conflict_proof_id: null,
      publication_conflict_detected_at_unix_seconds: null,
      detail: "authenticated-current-authority",
    },
    {
      peer_device_id_hex: "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
      primary: false,
      route_policy: 2,
      descriptor_path: "C:\\Kilogram\\peer-secondary.ticket",
      state: 2,
      authority_revision: null,
      publication_channel_id: "channel-m1129",
      observed_publication_generation: 3,
      observed_at_unix_seconds: 1800001990,
      publication_conflict_generation: 3,
      publication_conflict_proof_id: "proof-m1129",
      publication_conflict_detected_at_unix_seconds: 1800001998,
      detail: "signed-publication-conflict-proof",
    },
  ],
  history_snapshot_id_hex: "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeef",
  history_next_before_index: 1,
  history_messages: [
    {
      event_id_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
      author_account_id_hex: "29acbae141bccaf0b22e1a94d34d0bc7361e526d0bfe12c89794bc9322966dd7",
      author_device_id_hex: "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8",
      author_sequence: 1,
      body: "first visible message",
    },
    {
      event_id_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
      author_account_id_hex: "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
      author_device_id_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
      author_sequence: 2,
      body: "latest preview",
    },
  ],
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
assert(schema.milestone === "M1.1.31" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC schema header changed");
assert(schema.limits.maximum_read_response_bytes === 262144 &&
  schema.limits.maximum_conversation_page_size === 64 &&
  schema.limits.maximum_history_page_size === 100 &&
  schema.limits.maximum_history_page_body_bytes === 196608,
"runtime IPC read bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v26-with-json-descriptor-v7" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active" &&
  JSON.stringify(vectors.dependencies) === JSON.stringify(["CANONICAL-RUNTIME-IPC-VECTORS.json"]),
"runtime IPC read vector header changed");
equalJson(generate(core, vectors.input), vectors.expected,
  "independent canonical runtime IPC read vector");

if (options.selfTest) {
  const changedInput = structuredClone(vectors.input);
  changedInput.conversation_label = "m1129-read-model-tampered";
  const changed = generate(core, changedInput);
  assert(changed.conversation_list.response.signature_hex !==
    vectors.expected.conversation_list.response.signature_hex,
  "changed conversation summary preserved list response signature");
  assert(changed.history_page.request.object_id_hex !==
    vectors.expected.history_page.request.object_id_hex,
  "changed history label preserved request digest");
  assert(changed.history_page.response.signature_hex !==
    vectors.expected.history_page.response.signature_hex,
  "changed history request preserved response signature");
  const tampered = structuredClone(vectors.expected);
  tampered.conversation_list.response.signature_hex =
    `00${tampered.conversation_list.response.signature_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(core, vectors.input), tampered, "tampered read vector"); }
  catch { rejected = true; }
  assert(rejected, "tampered read response signature vector was accepted");
  process.stdout.write("changed_read_request_binding=rejected\n");
  process.stdout.write("tampered_read_response_signature=rejected\n");
  process.stdout.write("canonical_runtime_ipc_read_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_read_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("bounded_conversation_page=verified\n");
  process.stdout.write("snapshot_bound_history_page=verified\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
