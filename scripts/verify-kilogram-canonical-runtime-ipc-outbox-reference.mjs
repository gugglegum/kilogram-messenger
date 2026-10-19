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
  requestDigest: "kilogram:runtime-ipc-outbox-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-outbox-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-outbox-response-id:cbor-v1\0",
  snapshotId: "kilogram:runtime-ipc-outbox-snapshot-id:cbor-v1\0",
}).map(([name, value]) => [name, Buffer.from(value)]));
const STATE_TAGS = new Map([
  ["queued", 0],
  ["materialized", 1],
  ["delivered", 2],
  ["mailbox-pending", 3],
  ["mailbox-stored", 4],
  ["mailbox-expired", 5],
  ["mailbox-failed", 6],
]);

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
function objectJson(value, domain) {
  const id = domainHash(domain, value);
  return {
    cbor_hex: value.toString("hex"),
    id_domain_hex: domain.toString("hex"),
    id_preimage_hex: Buffer.concat([domain, value]).toString("hex"),
    object_id_hex: id.toString("hex"),
  };
}
function requestJson(value) {
  const output = objectJson(value, DOMAINS.requestDigest);
  delete output.id_domain_hex;
  output.digest_domain_hex = DOMAINS.requestDigest.toString("hex");
  return output;
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

function optionDigest(value, label) {
  return value === null ? array([uint(0)]) : array([uint(1), bytes(fromHex(value, 32, label))]);
}
function cursorOption(snapshotId, startIndex) {
  if (snapshotId === null) return array([uint(0)]);
  return array([uint(1), array([bytes(snapshotId), uint(startIndex)])]);
}
function encodeCounts(counts) {
  return array([
    uint(counts.contact_count), uint(counts.queue_count), uint(counts.pending_count),
    uint(counts.materialized_count), uint(counts.delivered_count),
    uint(counts.mailbox_pending_count), uint(counts.mailbox_stored_count),
    uint(counts.mailbox_expired_count), uint(counts.mailbox_failed_count),
    uint(counts.retry_record_count),
  ]);
}
function encodeItem(item) {
  assert(STATE_TAGS.has(item.state), `unknown outbox state ${item.state}`);
  return array([
    bytes(fromHex(item.queue_id_hex, 32, "queue ID")),
    bytes(fromHex(item.contact_id_hex, 32, "contact ID")),
    bytes(fromHex(item.peer_account_id_hex, 32, "peer Account ID")),
    bytes(fromHex(item.conversation_id_hex, 32, "Conversation ID")),
    uint(STATE_TAGS.get(item.state)),
    optionDigest(item.acknowledgement_event_id_hex, "acknowledgement event ID"),
    uint(item.retry_record_count),
  ]);
}
function validateSnapshot(input) {
  const { counts, items } = input;
  assert(items.length === counts.queue_count, "queue count must equal complete snapshot length");
  let previous = "";
  const actual = {
    pending_count: 0, materialized_count: 0, delivered_count: 0,
    mailbox_pending_count: 0, mailbox_stored_count: 0,
    mailbox_expired_count: 0, mailbox_failed_count: 0, retry_record_count: 0,
  };
  for (const item of items) {
    fromHex(item.queue_id_hex, 32, "queue ID");
    assert(previous < item.queue_id_hex, "outbox items must be strictly ordered by queue ID");
    previous = item.queue_id_hex;
    assert(STATE_TAGS.has(item.state), `unknown outbox state ${item.state}`);
    const delivered = item.state === "delivered";
    assert(delivered === (item.acknowledgement_event_id_hex !== null),
      "only delivered items carry acknowledgement IDs");
    if (item.acknowledgement_event_id_hex !== null)
      fromHex(item.acknowledgement_event_id_hex, 32, "acknowledgement event ID");
    if (!delivered) actual.pending_count += 1;
    if (item.state !== "queued") actual.materialized_count += 1;
    if (delivered) actual.delivered_count += 1;
    if (item.state === "mailbox-pending") actual.mailbox_pending_count += 1;
    if (item.state === "mailbox-stored") actual.mailbox_stored_count += 1;
    if (item.state === "mailbox-expired") actual.mailbox_expired_count += 1;
    if (item.state === "mailbox-failed") actual.mailbox_failed_count += 1;
    actual.retry_record_count += item.retry_record_count;
  }
  for (const [name, value] of Object.entries(actual))
    assert(counts[name] === value, `${name} does not match complete snapshot`);
}

function generate(core, queue, input) {
  validateSnapshot(input);
  const signer = key(core.input.device_secret_key_hex);
  assert(signer.publicKey.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  assert(input.items[0].queue_id_hex === queue.input.request_id_hex &&
    input.items[0].contact_id_hex === queue.expected.contact_id_hex &&
    input.items[0].conversation_id_hex === queue.expected.conversation_id_hex,
  "durable queue dependency changed");
  const descriptorId = fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID");
  const bearer = fromHex(core.input.bearer_token_hex, 32, "bearer token");
  const localAccount = fromHex(core.expected.account_id_hex, 32, "local Account ID");
  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const itemCbor = input.items.map(encodeItem);
  const snapshotCbor = array([uint(VERSION), encodeCounts(input.counts), array(itemCbor)]);
  const snapshot = objectJson(snapshotCbor, DOMAINS.snapshotId);
  const snapshotId = fromHex(snapshot.object_id_hex, 32, "snapshot ID");
  const command = array([uint(0), cursorOption(null, 0), uint(input.page_limit)]);
  const request = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds), command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);
  const pageItems = itemCbor.slice(0, input.page_limit);
  const nextIndex = pageItems.length;
  assert(nextIndex < input.items.length, "fixture must contain a continuation page");
  const page = array([
    bytes(snapshotId), encodeCounts(input.counts), array(pageItems),
    cursorOption(snapshotId, nextIndex),
  ]);
  const result = array([uint(0), page]);
  const content = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest),
    bytes(signer.publicKey), result,
  ]);
  const response = signed(DOMAINS.responseSignature, content, signer);
  return {
    account_id_hex: localAccount.toString("hex"),
    device_id_hex: signer.publicKey.toString("hex"),
    snapshot,
    request: requestJson(request),
    response: responseJson(response),
    page: {
      item_count: pageItems.length,
      next_start_index: nextIndex,
      snapshot_id_hex: snapshot.object_id_hex,
    },
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      snapshot_is_content_derived: true,
      pagination_is_snapshot_bound: true,
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
    else if (["--schema", "--vectors", "--core-vectors", "--queue-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const required of ["schema", "vectors", "core-vectors", "queue-vectors"])
    assert(result[required], `--${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
const queue = JSON.parse(readFileSync(options["queue-vectors"], "utf8"));
assert(schema.milestone === "M1.1.49" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC outbox schema header changed");
assert(schema.limits.maximum_outbox_request_bytes === 65536 &&
  schema.limits.maximum_outbox_response_bytes === 131072 &&
  schema.limits.maximum_outbox_page_size === 128 &&
  schema.limits.maximum_outbox_snapshot_items === 4096,
"runtime IPC outbox bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v32-with-json-descriptor-v13" &&
  schema.activation.current_runtime_changed === true, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active" &&
  JSON.stringify(vectors.dependencies) === JSON.stringify([
    "CANONICAL-RUNTIME-IPC-VECTORS.json",
    "CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json",
  ]), "runtime IPC outbox vector header changed");
equalJson(generate(core, queue, vectors.input), vectors.expected,
  "independent canonical runtime IPC outbox vector");

if (options.selfTest) {
  const changedItem = structuredClone(vectors.input);
  changedItem.items[0].retry_record_count = 1;
  changedItem.counts.retry_record_count += 1;
  assert(generate(core, queue, changedItem).snapshot.object_id_hex !==
    vectors.expected.snapshot.object_id_hex, "changed item preserved snapshot ID");
  const changedCounts = structuredClone(vectors.input);
  changedCounts.counts.pending_count += 1;
  let countsRejected = false;
  try { generate(core, queue, changedCounts); } catch { countsRejected = true; }
  assert(countsRejected, "inconsistent global counts were accepted");
  const changedCursor = structuredClone(vectors.input);
  changedCursor.page_limit = 1;
  const changedCursorOutput = generate(core, queue, changedCursor);
  assert(changedCursorOutput.page.next_start_index !== vectors.expected.page.next_start_index &&
    changedCursorOutput.response.signature_hex !== vectors.expected.response.signature_hex,
  "changed continuation preserved response signature");
  process.stdout.write("changed_outbox_item=rejected\n");
  process.stdout.write("changed_outbox_counts=rejected\n");
  process.stdout.write("changed_outbox_cursor=rejected\n");
  process.stdout.write("canonical_runtime_ipc_outbox_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_outbox_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("snapshot_bound_pagination=verified\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
