import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const PEER_REQUEST_DIGEST_DOMAIN = Buffer.from(
  "kilogram:blind-mailbox-peer-request:cbor-v1\0",
);
const READ_AUTHORIZATION_DOMAIN = Buffer.from("kilogram:blind-mailbox-read:cbor-v1\0");

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
function privateFromSeed(seed) {
  return createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]),
    format: "der",
    type: "pkcs8",
  });
}
function sign(content, domain, privateKey) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, privateKey);
  assert(verifyEd25519(null, preimage, createPublicKey(privateKey), signature),
    "self signature failed");
  return array([uint(VERSION), content, bytes(signature)]);
}
function peerRequest(operation, request) {
  return array([uint(VERSION), uint(operation), bytes(request)]);
}
function digest(request) {
  return blake3Hash(Buffer.concat([PEER_REQUEST_DIGEST_DOMAIN, request]));
}
function peerSuccess(operation, requestDigest, response) {
  return array([
    uint(VERSION), uint(operation), bytes(requestDigest),
    array([uint(VERSION), uint(1), response]),
  ]);
}
function peerRejected(operation, requestDigest, rejection) {
  return array([
    uint(VERSION), uint(operation), bytes(requestDigest),
    array([uint(VERSION), uint(2), uint(rejection)]),
  ]);
}
function hex(value) { return value.toString("hex"); }

function generate(mailbox) {
  assert(mailbox.schema_version === 1 && mailbox.profile === "kilogram-deterministic-cbor-v1",
    "canonical mailbox dependency header changed");
  const address = fromHex(mailbox.expected.address_cbor_hex, undefined, "address");
  const envelope = fromHex(mailbox.expected.envelope.cbor_hex, undefined, "envelope");
  const writeAuthorization = fromHex(
    mailbox.expected.write_authorization.signed_cbor_hex, undefined, "write authorization",
  );
  const listAuthorization = fromHex(
    mailbox.expected.read_list_authorization.signed_cbor_hex, undefined, "list authorization",
  );
  const storedReceipt = fromHex(
    mailbox.expected.stored_receipt.signed_cbor_hex, undefined, "stored receipt",
  );
  const deleteReceipt = fromHex(
    mailbox.expected.delete_receipt.signed_cbor_hex, undefined, "delete receipt",
  );
  const itemId = fromHex(mailbox.input.item_id_hex, 32, "item ID");
  const mailboxId = fromHex(mailbox.expected.mailbox_id_hex, 32, "mailbox ID");
  const readKey = fromHex(mailbox.expected.read_key_hex, 32, "read key");
  const storedReceiptId = fromHex(mailbox.expected.stored_receipt.id_hex, 32, "stored receipt ID");
  const readPrivate = privateFromSeed(fromHex(
    mailbox.input.read_secret_key_hex, 32, "read secret key",
  ));
  const deleteOperation = array([
    uint(VERSION), uint(2), bytes(itemId), bytes(storedReceiptId),
  ]);
  const deleteContent = array([
    uint(VERSION), bytes(mailboxId), bytes(readKey), deleteOperation,
  ]);
  const deleteAuthorization = sign(deleteContent, READ_AUTHORIZATION_DOMAIN, readPrivate);

  const putRequest = array([
    uint(VERSION), bytes(address), bytes(writeAuthorization), bytes(envelope),
  ]);
  const listRequest = array([
    uint(VERSION), bytes(address), bytes(listAuthorization),
  ]);
  const deleteRequest = array([
    uint(VERSION), bytes(address), bytes(deleteAuthorization),
  ]);
  const peerPut = peerRequest(1, putRequest);
  const peerList = peerRequest(2, listRequest);
  const peerDelete = peerRequest(3, deleteRequest);
  const putDigest = digest(peerPut);
  const listDigest = digest(peerList);
  const deleteDigest = digest(peerDelete);

  const putStored = array([uint(VERSION), uint(1), uint(1), bytes(storedReceipt)]);
  const putConflict = array([uint(VERSION), uint(2)]);
  const putTombstoned = array([uint(VERSION), uint(3)]);
  const putCapacity = array([uint(VERSION), uint(4)]);
  const storedItem = array([
    uint(VERSION), bytes(itemId), uint(mailbox.input.expires_at),
    bytes(envelope), bytes(storedReceipt),
  ]);
  const listResponse = array([
    uint(VERSION), array([storedItem]), array([]), uint(0),
  ]);
  const deleteDeleted = array([uint(VERSION), uint(1), uint(1), bytes(deleteReceipt)]);
  const deleteAbsent = array([uint(VERSION), uint(2)]);
  const deleteMismatch = array([uint(VERSION), uint(3)]);
  const deleteCapacity = array([uint(VERSION), uint(4)]);

  return {
    peer_request_digest_domain_hex: hex(PEER_REQUEST_DIGEST_DOMAIN),
    response_variants: {
      put_conflict_cbor_hex: hex(putConflict),
      put_tombstoned_cbor_hex: hex(putTombstoned),
      put_capacity_exceeded_cbor_hex: hex(putCapacity),
      delete_absent_cbor_hex: hex(deleteAbsent),
      delete_receipt_mismatch_cbor_hex: hex(deleteMismatch),
      delete_tombstone_capacity_exceeded_cbor_hex: hex(deleteCapacity),
    },
    list_response_cbor_hex: hex(listResponse),
    peer_requests: {
      put: { cbor_hex: hex(peerPut), digest_hex: hex(putDigest) },
      list: { cbor_hex: hex(peerList), digest_hex: hex(listDigest) },
      delete: { cbor_hex: hex(peerDelete), digest_hex: hex(deleteDigest) },
    },
    peer_responses: {
      put_success_cbor_hex: hex(peerSuccess(1, putDigest, putStored)),
      list_success_cbor_hex: hex(peerSuccess(2, listDigest, listResponse)),
      delete_success_cbor_hex: hex(peerSuccess(3, deleteDigest, deleteDeleted)),
      put_rejected_cbor_hex: hex(peerRejected(1, putDigest, 1)),
      list_rejected_cbor_hex: hex(peerRejected(2, listDigest, 2)),
      delete_rejected_cbor_hex: hex(peerRejected(3, deleteDigest, 4)),
    },
  };
}

function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--mailbox-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["mailbox-vectors"], "--mailbox-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const options = parse(process.argv.slice(2));
const mailbox = JSON.parse(readFileSync(options["mailbox-vectors"], "utf8"));
const expected = generate(mailbox);
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependency: "CANONICAL-MAILBOX-VECTORS.json",
    input: {
      store_key_hex: mailbox.expected.store_key_hex,
      mailbox_id_hex: mailbox.expected.mailbox_id_hex,
      item_id_hex: mailbox.input.item_id_hex,
      expires_at: mailbox.input.expires_at,
    },
    expected,
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.16" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.cddl.path === "CANONICAL-MAILBOX-TRANSPORT.cddl" &&
  schema.cddl.root_rule === "kilogram-mailbox-transport-v1", "schema CDDL binding changed");
assert(schema.activation.current_runtime_encoding === "postcard" &&
  schema.activation.current_runtime_alpn === "kilogram/m0/blind-mailbox/1" &&
  schema.activation.current_runtime_changed === false &&
  schema.activation.candidate_protocol_identifier === "unassigned", "runtime activation changed");
assert(vectors.dependency === "CANONICAL-MAILBOX-VECTORS.json", "vector dependency changed");
equalJson(expected, vectors.expected, "canonical mailbox transport vectors");

if (options.selfTest) {
  const changed = structuredClone(vectors.expected);
  changed.peer_requests.put.digest_hex = `ff${changed.peer_requests.put.digest_hex.slice(2)}`;
  assert(JSON.stringify(changed) !== JSON.stringify(expected), "changed request digest was accepted");
  const changedResponse = structuredClone(vectors.expected);
  changedResponse.peer_responses.put_rejected_cbor_hex =
    changedResponse.peer_responses.put_rejected_cbor_hex.replace(/.$/, "5");
  assert(JSON.stringify(changedResponse) !== JSON.stringify(expected), "changed rejection was accepted");
  process.stdout.write("changed_peer_request_digest=rejected\n");
  process.stdout.write("changed_peer_rejection=rejected\n");
  process.stdout.write("canonical_mailbox_transport_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_mailbox_transport_reference=verified\n");
