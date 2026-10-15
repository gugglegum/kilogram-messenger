import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";

const PROFILE_VERSION = 1;
const PKCS8_ED25519_SEED_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_ED25519_PUBLIC_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const SIGNATURE_DOMAIN = Buffer.from(
  "kilogram:history-recovery-checkpoint-signature:cbor-v1\0",
  "utf8",
);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function hex(value, expectedBytes, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} is not lowercase hexadecimal`);
  const result = Buffer.from(value, "hex");
  if (expectedBytes !== undefined) assert(result.length === expectedBytes, `${label} length changed`);
  return result;
}

function head(major, value) {
  assert(Number.isSafeInteger(value) && value >= 0, "CBOR value is outside the safe integer range");
  const prefix = major << 5;
  if (value <= 23) return Buffer.from([prefix | value]);
  if (value <= 0xff) return Buffer.from([prefix | 24, value]);
  if (value <= 0xffff) {
    const out = Buffer.alloc(3); out[0] = prefix | 25; out.writeUInt16BE(value, 1); return out;
  }
  if (value <= 0xffffffff) {
    const out = Buffer.alloc(5); out[0] = prefix | 26; out.writeUInt32BE(value, 1); return out;
  }
  const out = Buffer.alloc(9); out[0] = prefix | 27; out.writeBigUInt64BE(BigInt(value), 1); return out;
}

const uint = (value) => head(0, value);
const bytes = (value) => Buffer.concat([head(2, value.length), value]);
const array = (values) => Buffer.concat([head(4, values.length), ...values]);

function privateKey(seed) {
  return createPrivateKey({
    key: Buffer.concat([PKCS8_ED25519_SEED_PREFIX, seed]),
    format: "der",
    type: "pkcs8",
  });
}

function publicBytes(key) {
  const spki = createPublicKey(key).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_ED25519_PUBLIC_PREFIX.length).equals(SPKI_ED25519_PUBLIC_PREFIX),
    "unexpected Ed25519 SPKI prefix");
  return spki.subarray(SPKI_ED25519_PUBLIC_PREFIX.length);
}

function checkpointContent(input, recipient, inventory, next, previous) {
  return array([
    uint(PROFILE_VERSION),
    bytes(hex(input.account_id_hex, 32, "account ID")),
    bytes(hex(input.conversation_id_hex, 32, "conversation ID")),
    bytes(hex(input.source_device_id_hex, 32, "source Device ID")),
    bytes(recipient),
    bytes(hex(input.sas_hex, 32, "SAS")),
    uint(input.approved_range_start),
    uint(input.approved_range_end),
    uint(input.page_size),
    inventory === undefined
      ? array([])
      : array([uint(inventory.count), bytes(inventory.digest)]),
    uint(next),
    previous === undefined ? array([]) : array([bytes(previous)]),
  ]);
}

function verifyCheckpoint(name, expected, content, key, publicKey) {
  assert(content.equals(hex(expected.content_cbor_hex, undefined, `${name} content`)),
    `${name} content diverged`);
  assert(SIGNATURE_DOMAIN.equals(hex(expected.signature_domain_hex, undefined, `${name} domain`)),
    `${name} signature domain diverged`);
  const preimage = Buffer.concat([SIGNATURE_DOMAIN, content]);
  assert(preimage.equals(hex(expected.signing_preimage_hex, undefined, `${name} preimage`)),
    `${name} preimage diverged`);
  const signature = hex(expected.signature_hex, 64, `${name} signature`);
  assert(signEd25519(null, preimage, key).equals(signature), `${name} deterministic signature diverged`);
  assert(verifyEd25519(null, preimage, publicKey, signature), `${name} signature is invalid`);
  const envelope = array([uint(PROFILE_VERSION), content, bytes(signature)]);
  assert(envelope.equals(hex(expected.signed_cbor_hex, undefined, `${name} envelope`)),
    `${name} signed envelope diverged`);
  return envelope;
}

const path = process.argv[2] ?? new URL("../CANONICAL-RECOVERY-VECTORS.json", import.meta.url);
const vectors = JSON.parse(readFileSync(path, "utf8"));
assert(vectors.schema_version === 1, "vector schema version changed");
assert(vectors.profile === "kilogram-deterministic-cbor-v1", "vector profile changed");
assert(vectors.activation_status === "not-active", "vectors unexpectedly claim activation");

const input = vectors.input;
const expected = vectors.expected;
const recipientKey = privateKey(hex(input.recipient_secret_key_hex, 32, "recipient seed"));
const recipient = publicBytes(recipientKey);
assert(recipient.equals(hex(expected.recipient_device_id_hex, 32, "recipient Device ID")),
  "recipient Device ID diverged");
const initialContent = checkpointContent(
  input,
  recipient,
  undefined,
  input.approved_range_start,
  undefined,
);
verifyCheckpoint("initial checkpoint", expected.initial_checkpoint, initialContent,
  recipientKey, createPublicKey(recipientKey));
const predecessor = hex(expected.initial_checkpoint.checkpoint_id_hex, 32, "predecessor ID");
const successorContent = checkpointContent(
  input,
  recipient,
  { count: input.inventory_event_count, digest: hex(input.inventory_digest_hex, 32, "inventory digest") },
  input.successor_next_range_start,
  predecessor,
);
verifyCheckpoint("successor checkpoint", expected.successor_checkpoint, successorContent,
  recipientKey, createPublicKey(recipientKey));
const identity = array([
  uint(PROFILE_VERSION),
  bytes(hex(input.account_id_hex, 32, "account ID")),
  bytes(hex(input.conversation_id_hex, 32, "conversation ID")),
  bytes(hex(input.source_device_id_hex, 32, "source Device ID")),
  bytes(recipient),
  bytes(hex(input.sas_hex, 32, "SAS")),
  uint(input.approved_range_start),
  uint(input.approved_range_end),
  uint(input.page_size),
]);
assert(identity.equals(hex(expected.recovery_identity_cbor_hex, undefined, "recovery identity")),
  "recovery identity diverged");

console.log("canonical_recovery_reference=verified");
console.log("independent_language=node-javascript");
console.log("independent_ed25519=node-crypto");
console.log("independent_blake3=pending-m1.1.5");
console.log("active_runtime_changed=false");
