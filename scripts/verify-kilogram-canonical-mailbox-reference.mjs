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
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = {
  mailboxId: Buffer.from("kilogram:blind-mailbox-id:cbor-v1\0"),
  hpkeInfo: Buffer.from("kilogram:blind-mailbox-envelope:cbor-v1\0"),
  envelopeDigest: Buffer.from("kilogram:blind-mailbox-envelope-digest:cbor-v1\0"),
  write: Buffer.from("kilogram:blind-mailbox-write:cbor-v1\0"),
  read: Buffer.from("kilogram:blind-mailbox-read:cbor-v1\0"),
  offer: Buffer.from("kilogram:blind-mailbox-storage-offer:cbor-v1\0"),
  offerWork: Buffer.from("kilogram:blind-mailbox-storage-offer-admission-work:cbor-v1\0"),
  stored: Buffer.from("kilogram:blind-mailbox-stored-receipt:cbor-v1\0"),
  storedId: Buffer.from("kilogram:blind-mailbox-receipt-id:cbor-v1\0"),
  deleted: Buffer.from("kilogram:blind-mailbox-delete-receipt:cbor-v1\0"),
  deletedId: Buffer.from("kilogram:blind-mailbox-delete-id:cbor-v1\0"),
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
function privateFromSeed(seed) {
  return createPrivateKey({ key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8" });
}
function publicBytes(privateKey) {
  const spki = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI prefix");
  return spki.subarray(SPKI_PREFIX.length);
}
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function sign(content, domain, privateKey, idDomain) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, privateKey);
  assert(verifyEd25519(null, preimage, createPublicKey(privateKey), signature), "self signature failed");
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  return {
    content,
    preimage,
    signature,
    envelope,
    id: idDomain === undefined ? undefined : domainHash(idDomain, envelope),
    domain,
  };
}
function expectedSigned(value) {
  const output = {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: value.domain.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
  };
  if (value.id !== undefined) output.id_hex = value.id.toString("hex");
  return output;
}
function increment(value) {
  for (let index = value.length - 1; index >= 0; index -= 1) {
    value[index] = (value[index] + 1) & 0xff;
    if (value[index] !== 0) break;
  }
}
function leadingZeroBits(value) {
  let bits = 0;
  for (const byte of value) {
    if (byte === 0) bits += 8;
    else {
      for (let bit = 7; bit >= 0 && ((byte >>> bit) & 1) === 0; bit -= 1) bits += 1;
      break;
    }
  }
  return bits;
}

function generate(input) {
  const readPrivate = privateFromSeed(fromHex(input.read_secret_key_hex, 32, "read seed"));
  const writePrivate = privateFromSeed(fromHex(input.write_secret_key_hex, 32, "write seed"));
  const storePrivate = privateFromSeed(fromHex(input.store_secret_key_hex, 32, "store seed"));
  const readKey = publicBytes(readPrivate);
  const writeKey = publicBytes(writePrivate);
  const storeKey = publicBytes(storePrivate);
  const itemId = fromHex(input.item_id_hex, 32, "item ID");
  const requestNonce = fromHex(input.request_nonce_hex, 32, "request nonce");

  const address = array([uint(VERSION), bytes(readKey), bytes(writeKey)]);
  const mailboxId = domainHash(DOMAINS.mailboxId, address);
  const envelopeContext = array([
    uint(VERSION), bytes(mailboxId), bytes(itemId), uint(input.created_at), uint(input.expires_at),
  ]);
  const encapsulatedKey = fromHex(input.encapsulated_key_hex, 32, "encapsulated key");
  const ciphertext = fromHex(input.ciphertext_hex, undefined, "ciphertext");
  const envelope = array([
    uint(VERSION), envelopeContext, bytes(encapsulatedKey), bytes(ciphertext),
  ]);
  const envelopeDigest = domainHash(DOMAINS.envelopeDigest, envelope);

  const writeContent = array([
    uint(VERSION), bytes(mailboxId), bytes(itemId), bytes(writeKey),
    uint(input.requested_ttl_seconds), uint(envelope.length), bytes(envelopeDigest),
  ]);
  const writeAuthorization = sign(writeContent, DOMAINS.write, writePrivate);
  const readListOperation = array([
    uint(VERSION), uint(1), bytes(requestNonce), array([]), uint(input.page_limit),
  ]);
  const readListContent = array([
    uint(VERSION), bytes(mailboxId), bytes(readKey), readListOperation,
  ]);
  const readListAuthorization = sign(readListContent, DOMAINS.read, readPrivate);
  const placeholderReceipt = fromHex(input.delete_receipt_id_hex, 32, "delete receipt ID");
  const readDeleteOperation = array([
    uint(VERSION), uint(2), bytes(itemId), bytes(placeholderReceipt),
  ]);
  const readDeleteContent = array([
    uint(VERSION), bytes(mailboxId), bytes(readKey), readDeleteOperation,
  ]);
  const readDeleteAuthorization = sign(readDeleteContent, DOMAINS.read, readPrivate);

  const providerEndpoint = Buffer.from(input.provider_endpoint, "utf8");
  const offerNonce = Buffer.alloc(32);
  let offerContent;
  let workBits;
  for (;;) {
    offerContent = array([
      uint(VERSION), bytes(providerEndpoint), bytes(storeKey), uint(1),
      uint(input.capacity_hint_bytes), uint(input.max_record_bytes), uint(input.offer_issued_at),
      uint(input.offer_expires_at), bytes(offerNonce),
    ]);
    workBits = leadingZeroBits(domainHash(DOMAINS.offerWork, offerContent));
    if (workBits >= input.minimum_work_bits) break;
    increment(offerNonce);
  }
  const storageOffer = sign(offerContent, DOMAINS.offer, storePrivate);

  const storedContent = array([
    uint(VERSION), bytes(storeKey), bytes(mailboxId), bytes(itemId), uint(envelope.length),
    bytes(envelopeDigest), uint(input.stored_at), uint(input.expires_at),
  ]);
  const storedReceipt = sign(storedContent, DOMAINS.stored, storePrivate, DOMAINS.storedId);
  const deleteContent = array([
    uint(VERSION), bytes(storeKey), bytes(mailboxId), bytes(itemId), bytes(storedReceipt.id),
    uint(input.deleted_at), uint(input.expires_at),
  ]);
  const deleteReceipt = sign(deleteContent, DOMAINS.deleted, storePrivate, DOMAINS.deletedId);

  return {
    read_key_hex: readKey.toString("hex"),
    write_key_hex: writeKey.toString("hex"),
    store_key_hex: storeKey.toString("hex"),
    address_cbor_hex: address.toString("hex"),
    mailbox_id_domain_hex: DOMAINS.mailboxId.toString("hex"),
    mailbox_id_hex: mailboxId.toString("hex"),
    envelope: {
      hpke_info_hex: DOMAINS.hpkeInfo.toString("hex"),
      aad_cbor_hex: envelopeContext.toString("hex"),
      cbor_hex: envelope.toString("hex"),
      digest_domain_hex: DOMAINS.envelopeDigest.toString("hex"),
      digest_hex: envelopeDigest.toString("hex"),
      bytes: envelope.length,
    },
    write_authorization: expectedSigned(writeAuthorization),
    read_list_authorization: expectedSigned(readListAuthorization),
    read_delete_authorization: expectedSigned(readDeleteAuthorization),
    storage_offer: {
      ...expectedSigned(storageOffer),
      admission_work_domain_hex: DOMAINS.offerWork.toString("hex"),
      admission_work_bits: workBits,
      nonce_hex: offerNonce.toString("hex"),
    },
    stored_receipt: expectedSigned(storedReceipt),
    delete_receipt: expectedSigned(deleteReceipt),
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
    else if (["--schema", "--vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  read_secret_key_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  write_secret_key_hex: "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
  store_secret_key_hex: "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
  item_id_hex: "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
  request_nonce_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  delete_receipt_id_hex: "b0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecf",
  encapsulated_key_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  ciphertext_hex: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
  created_at: 1800000000,
  expires_at: 1800000600,
  requested_ttl_seconds: 600,
  page_limit: 8,
  provider_endpoint: "iroh-ticket:opaque-provider-endpoint",
  capacity_hint_bytes: 209715200,
  max_record_bytes: 1048576,
  offer_issued_at: 1800000000,
  offer_expires_at: 1800000300,
  minimum_work_bits: 8,
  stored_at: 1800000020,
  deleted_at: 1800000300,
};

const options = parse(process.argv.slice(2));
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.15" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.cddl.path === "CANONICAL-MAILBOX.cddl" &&
  schema.cddl.root_rule === "kilogram-mailbox-v1", "schema CDDL binding changed");
assert(schema.activation.current_runtime_changed === false &&
  schema.activation.candidate_protocol_identifier === "unassigned", "runtime activation changed");
assert(schema.registries.read_operation["1"] === "list" &&
  schema.registries.read_operation["2"] === "delete" &&
  schema.registries.storage_policy_class["1"] === "bounded-volunteer", "closed registry changed");
equalJson(generate(vectors.input), vectors.expected, "canonical mailbox vectors");

if (options.selfTest) {
  const changedKey = structuredClone(vectors.input);
  changedKey.write_secret_key_hex = `ff${changedKey.write_secret_key_hex.slice(2)}`;
  assert(JSON.stringify(generate(changedKey)) !== JSON.stringify(vectors.expected),
    "changed write capability unexpectedly preserved vectors");
  const changedCiphertext = structuredClone(vectors.input);
  changedCiphertext.ciphertext_hex = `ff${changedCiphertext.ciphertext_hex.slice(2)}`;
  assert(JSON.stringify(generate(changedCiphertext)) !== JSON.stringify(vectors.expected),
    "changed ciphertext unexpectedly preserved vectors");
  process.stdout.write("changed_write_capability=rejected\n");
  process.stdout.write("changed_ciphertext=rejected\n");
  process.stdout.write("canonical_mailbox_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_mailbox_reference=verified\n");
