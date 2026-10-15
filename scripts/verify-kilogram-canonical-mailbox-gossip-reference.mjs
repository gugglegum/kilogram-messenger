import { createPublicKey, verify as verifyEd25519 } from "node:crypto";
import { readFileSync } from "node:fs";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = {
  offerId: Buffer.from("kilogram:mailbox-provider-offer-id:cbor-v1\0"),
  selection: Buffer.from("kilogram:mailbox-provider-selection:cbor-v1\0"),
  gossipSelection: Buffer.from("kilogram:mailbox-provider-gossip-selection:cbor-v1\0"),
  frameId: Buffer.from("kilogram:mailbox-provider-gossip-frame-id:cbor-v1\0"),
};

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
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function verifyMailboxDependency(mailbox) {
  const offer = mailbox.expected.storage_offer;
  const storeKey = fromHex(mailbox.expected.store_key_hex, 32, "store key");
  const signature = fromHex(offer.signature_hex, 64, "offer signature");
  const preimage = fromHex(offer.signing_preimage_hex, undefined, "offer signing preimage");
  const publicKey = createPublicKey({
    key: Buffer.concat([SPKI_PREFIX, storeKey]), format: "der", type: "spki",
  });
  assert(verifyEd25519(null, preimage, publicKey, signature), "mailbox offer signature failed");
  assert(offer.signed_cbor_hex.startsWith("8301"), "mailbox offer is not the expected canonical envelope");
}
function generate(input, mailbox) {
  verifyMailboxDependency(mailbox);
  const offer = fromHex(mailbox.expected.storage_offer.signed_cbor_hex, undefined, "storage offer");
  const storeKey = fromHex(mailbox.expected.store_key_hex, 32, "store key");
  const transportIdentity = fromHex(input.transport_identity_hex, 32, "transport identity");
  const selectionSalt = fromHex(input.selection_salt_hex, 32, "selection salt");
  const requestEntropy = fromHex(input.request_entropy_hex, 32, "request entropy");
  const replyEntropy = fromHex(input.reply_entropy_hex, 32, "reply entropy");
  assert(offer.length <= 2048, "offer exceeds gossip bound");
  assert(mailbox.expected.storage_offer.admission_work_bits >= 8, "offer lacks admission work");

  const offerId = domainHash(DOMAINS.offerId, offer);
  const selectionTranscript = array([
    uint(VERSION), bytes(selectionSalt), bytes(storeKey), bytes(transportIdentity), bytes(offerId),
  ]);
  const selectionRank = domainHash(DOMAINS.selection, selectionTranscript);
  const gossipSelectionRank = domainHash(DOMAINS.gossipSelection, selectionTranscript);
  const requestEntry = array([uint(VERSION), uint(1), bytes(offer)]);
  const request = array([
    uint(VERSION), uint(input.request_created_at), uint(input.request_expires_at),
    bytes(requestEntropy), array([]), array([requestEntry]),
  ]);
  const requestId = domainHash(DOMAINS.frameId, request);
  const replyEntry = array([uint(VERSION), uint(2), bytes(offer)]);
  const reply = array([
    uint(VERSION), uint(input.reply_created_at), uint(input.reply_expires_at),
    bytes(replyEntropy), array([bytes(requestId)]), array([replyEntry]),
  ]);
  const replyId = domainHash(DOMAINS.frameId, reply);
  return {
    domains: Object.fromEntries(Object.entries(DOMAINS).map(([key, value]) => [key, value.toString("hex")])),
    offer_id_hex: offerId.toString("hex"),
    selection_transcript_cbor_hex: selectionTranscript.toString("hex"),
    selection_rank_hex: selectionRank.toString("hex"),
    gossip_selection_rank_hex: gossipSelectionRank.toString("hex"),
    request: {
      entry_cbor_hex: requestEntry.toString("hex"),
      frame_cbor_hex: request.toString("hex"),
      frame_id_hex: requestId.toString("hex"),
      frame_bytes: request.length,
    },
    reply: {
      entry_cbor_hex: replyEntry.toString("hex"),
      frame_cbor_hex: reply.toString("hex"),
      frame_id_hex: replyId.toString("hex"),
      frame_bytes: reply.length,
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
      result[argument.slice(2).replace("-", "_")] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result.mailbox_vectors, "--mailbox-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  transport_identity_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  selection_salt_hex: "707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f",
  request_entropy_hex: "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
  reply_entropy_hex: "909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeaf",
  request_created_at: 1800000030,
  request_expires_at: 1800000090,
  reply_created_at: 1800000031,
  reply_expires_at: 1800000091,
};

const options = parse(process.argv.slice(2));
const mailbox = JSON.parse(readFileSync(options.mailbox_vectors, "utf8"));
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependency: "CANONICAL-MAILBOX-VECTORS.json",
    input: defaultInput,
    expected: generate(defaultInput, mailbox),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.17" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.cddl.path === "CANONICAL-MAILBOX-GOSSIP.cddl" &&
  schema.cddl.root_rule === "kilogram-mailbox-provider-gossip-v1", "schema CDDL binding changed");
assert(schema.activation.current_runtime_encoding === "postcard" &&
  schema.activation.current_runtime_changed === false, "active provider gossip changed");
equalJson(generate(vectors.input, mailbox), vectors.expected, "canonical mailbox gossip vectors");

if (options.selfTest) {
  const changedEntropy = structuredClone(vectors.input);
  changedEntropy.request_entropy_hex = `ff${changedEntropy.request_entropy_hex.slice(2)}`;
  assert(generate(changedEntropy, mailbox).request.frame_id_hex !== vectors.expected.request.frame_id_hex,
    "changed entropy preserved request frame ID");
  const changedTransport = structuredClone(vectors.input);
  changedTransport.transport_identity_hex = `ff${changedTransport.transport_identity_hex.slice(2)}`;
  assert(generate(changedTransport, mailbox).selection_rank_hex !== vectors.expected.selection_rank_hex,
    "changed transport identity preserved selection rank");
  process.stdout.write("changed_entropy=rejected\n");
  process.stdout.write("changed_transport_identity=rejected\n");
  process.stdout.write("canonical_mailbox_gossip_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("nested_offer_signature=verified\n");
process.stdout.write("canonical_mailbox_gossip_reference=verified\n");
