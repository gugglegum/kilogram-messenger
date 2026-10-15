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
const LEGACY_HTTPS = 1;
const EXACT_VOLUNTEER = 2;
const LOCAL_PURPOSE = 1;
const OFFER_PURPOSE = 2;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = {
  bindingId: Buffer.from("kilogram:mailbox-binding-id:cbor-v1\0"),
  localSignature: Buffer.from("kilogram:mailbox-local-binding:cbor-v1\0"),
  localHpkeInfo: Buffer.from("kilogram:mailbox-local-binding-hpke:cbor-v1\0"),
  offerSignature: Buffer.from("kilogram:mailbox-peer-offer:cbor-v1\0"),
  offerHpkeInfo: Buffer.from("kilogram:mailbox-peer-offer-hpke:cbor-v1\0"),
  replicaSetId: Buffer.from("kilogram:mailbox-replica-set-commitment-id:cbor-v1\0"),
  updateSignature: Buffer.from("kilogram:mailbox-capability-update:cbor-v1\0"),
  updateId: Buffer.from("kilogram:mailbox-capability-update-id:cbor-v1\0"),
  acknowledgementSignature: Buffer.from("kilogram:mailbox-capability-acknowledgement:cbor-v1\0"),
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
const option = (value) => value === undefined ? array([]) : array([value]);
const bool = (value) => uint(value ? 1 : 0);

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function privateKey(seed) {
  return createPrivateKey({ key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8" });
}
function publicBytes(key) {
  const spki = createPublicKey(key).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI prefix");
  return spki.subarray(SPKI_PREFIX.length);
}
function sign(domain, content, key) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, key);
  assert(verifyEd25519(null, preimage, createPublicKey(key), signature), "generated signature failed");
  return {
    content_cbor_hex: content.toString("hex"),
    signature_domain_hex: domain.toString("hex"),
    signing_preimage_hex: preimage.toString("hex"),
    signature_hex: signature.toString("hex"),
    signed_cbor_hex: array([uint(VERSION), bytes(content), bytes(signature)]).toString("hex"),
  };
}
function domainHash(domain, encoded) { return blake3Hash(Buffer.concat([domain, encoded])); }
function service(url, storeKey) {
  return array([uint(VERSION), bytes(Buffer.from(url, "utf8")), bytes(storeKey)]);
}
function core(mode, ownerAccount, ownerDevice, peerAccount, peerDevice, scope, address, serviceValue,
  createdAt) {
  return array([
    uint(VERSION), uint(mode), bytes(ownerAccount), bytes(ownerDevice), bytes(peerAccount),
    bytes(peerDevice), bytes(scope), bytes(address), option(serviceValue), uint(createdAt),
  ]);
}
function header(purpose, bindingId) {
  return array([uint(VERSION), uint(purpose), bytes(bindingId)]);
}
function sealedEnvelope(headerBytes, encapsulatedKey, ciphertext) {
  return array([uint(VERSION), bytes(headerBytes), bytes(encapsulatedKey), bytes(ciphertext)]);
}
function replicaSet(storeKeys) {
  return array([uint(VERSION), array(storeKeys.map(bytes))]);
}
function expectedSigned(value) { return value; }

function generate(input, mailbox) {
  const ownerKey = privateKey(fromHex(input.owner_secret_key_hex, 32, "owner seed"));
  const recipientKey = privateKey(fromHex(input.recipient_secret_key_hex, 32, "recipient seed"));
  const ownerDevice = publicBytes(ownerKey);
  const recipientDevice = publicBytes(recipientKey);
  const ownerAccount = fromHex(input.owner_account_id_hex, 32, "owner account");
  const recipientAccount = fromHex(input.recipient_account_id_hex, 32, "recipient account");
  const scope = fromHex(input.scope_hex, 32, "scope");
  const ownerEncryption = fromHex(input.owner_encryption_public_key_hex, 32, "owner encryption key");
  const readSecret = fromHex(mailbox.input.read_secret_key_hex, 32, "read secret");
  const writeSecret = fromHex(mailbox.input.write_secret_key_hex, 32, "write secret");
  const address = fromHex(mailbox.expected.address_cbor_hex, undefined, "mailbox address");
  const readKey = fromHex(mailbox.expected.read_key_hex, 32, "read key");
  const writeKey = fromHex(mailbox.expected.write_key_hex, 32, "write key");
  assert(publicBytes(privateKey(readSecret)).equals(readKey), "read secret/address relation changed");
  assert(publicBytes(privateKey(writeSecret)).equals(writeKey), "write secret/address relation changed");
  const storeKeys = [
    fromHex(mailbox.expected.store_key_hex, 32, "store key"),
    fromHex(input.replica_store_key_2_hex, 32, "replica store key 2"),
    fromHex(input.replica_store_key_3_hex, 32, "replica store key 3"),
  ].sort(Buffer.compare);

  const legacyService = service(input.legacy_service_url, storeKeys[0]);
  const legacyCore = core(LEGACY_HTTPS, ownerAccount, ownerDevice, recipientAccount,
    recipientDevice, scope, address, legacyService, input.created_at);
  const exactCore = core(EXACT_VOLUNTEER, ownerAccount, ownerDevice, recipientAccount,
    recipientDevice, scope, address, undefined, input.created_at);
  const exactBindingId = domainHash(DOMAINS.bindingId, exactCore);
  const legacyBindingId = domainHash(DOMAINS.bindingId, legacyCore);

  const localContent = array([uint(VERSION), bytes(exactCore), bytes(readSecret), bytes(writeSecret)]);
  const signedLocal = sign(DOMAINS.localSignature, localContent, ownerKey);
  const localHeader = header(LOCAL_PURPOSE, exactBindingId);
  const localEnvelope = sealedEnvelope(localHeader,
    fromHex(input.local_encapsulated_key_hex, 32, "local encapsulated key"),
    fromHex(input.local_ciphertext_hex, undefined, "local ciphertext"));

  const legacyOfferContent = array([
    uint(VERSION), bytes(legacyCore), bytes(ownerEncryption), bytes(writeSecret), uint(input.expires_at),
  ]);
  const exactOfferContent = array([
    uint(VERSION), bytes(exactCore), bytes(ownerEncryption), bytes(writeSecret), uint(input.expires_at),
  ]);
  const signedLegacyOffer = sign(DOMAINS.offerSignature, legacyOfferContent, ownerKey);
  const signedExactOffer = sign(DOMAINS.offerSignature, exactOfferContent, ownerKey);
  const offerHeader = header(OFFER_PURPOSE, exactBindingId);
  const offerEnvelope = sealedEnvelope(offerHeader,
    fromHex(input.offer_encapsulated_key_hex, 32, "offer encapsulated key"),
    fromHex(input.offer_ciphertext_hex, undefined, "offer ciphertext"));

  const replica = replicaSet(storeKeys);
  const replicaTranscript = array([uint(VERSION), bytes(exactBindingId), bytes(replica)]);
  const replicaSetId = domainHash(DOMAINS.replicaSetId, replicaTranscript);
  const activationAction = array([
    uint(VERSION), uint(1), uint(EXACT_VOLUNTEER), bytes(exactBindingId), bytes(offerEnvelope),
    option(bytes(replica)),
  ]);
  const activationContent = array([
    uint(VERSION), bytes(ownerAccount), bytes(ownerDevice), bytes(recipientAccount),
    bytes(recipientDevice), bytes(scope), uint(1), option(undefined), uint(input.created_at),
    bytes(activationAction),
  ]);
  const activation = sign(DOMAINS.updateSignature, activationContent, ownerKey);
  const activationBytes = fromHex(activation.signed_cbor_hex, undefined, "activation update");
  const activationId = domainHash(DOMAINS.updateId, activationBytes);
  const revokeAction = array([uint(VERSION), uint(2), bytes(exactBindingId)]);
  const revokeContent = array([
    uint(VERSION), bytes(ownerAccount), bytes(ownerDevice), bytes(recipientAccount),
    bytes(recipientDevice), bytes(scope), uint(2), option(bytes(activationId)),
    uint(input.revoked_at), bytes(revokeAction),
  ]);
  const revocation = sign(DOMAINS.updateSignature, revokeContent, ownerKey);
  const revocationId = domainHash(DOMAINS.updateId,
    fromHex(revocation.signed_cbor_hex, undefined, "revocation update"));
  const sessionBinding = fromHex(input.session_binding_hex, 32, "session binding");
  const acknowledgementContent = array([
    uint(VERSION), bytes(sessionBinding), bytes(activationId), bytes(ownerAccount), bytes(ownerDevice),
    bytes(recipientAccount), bytes(recipientDevice), uint(1), bytes(exactBindingId), bool(false),
  ]);
  const acknowledgement = sign(DOMAINS.acknowledgementSignature,
    acknowledgementContent, recipientKey);

  return {
    domains: Object.fromEntries(Object.entries(DOMAINS).map(([name, value]) => [name, value.toString("hex")])),
    owner_device_id_hex: ownerDevice.toString("hex"),
    recipient_device_id_hex: recipientDevice.toString("hex"),
    legacy_core_cbor_hex: legacyCore.toString("hex"),
    legacy_binding_id_hex: legacyBindingId.toString("hex"),
    exact_core_cbor_hex: exactCore.toString("hex"),
    exact_binding_id_hex: exactBindingId.toString("hex"),
    signed_local_binding: expectedSigned(signedLocal),
    local_envelope: {
      header_cbor_hex: localHeader.toString("hex"),
      aad_cbor_hex: localHeader.toString("hex"),
      hpke_info_hex: DOMAINS.localHpkeInfo.toString("hex"),
      cbor_hex: localEnvelope.toString("hex"),
    },
    signed_legacy_peer_offer: expectedSigned(signedLegacyOffer),
    signed_exact_peer_offer: expectedSigned(signedExactOffer),
    offer_envelope: {
      header_cbor_hex: offerHeader.toString("hex"),
      aad_cbor_hex: offerHeader.toString("hex"),
      hpke_info_hex: DOMAINS.offerHpkeInfo.toString("hex"),
      cbor_hex: offerEnvelope.toString("hex"),
    },
    replica_set: {
      ordered_store_keys_hex: storeKeys.map((value) => value.toString("hex")),
      cbor_hex: replica.toString("hex"),
      commitment_transcript_cbor_hex: replicaTranscript.toString("hex"),
      commitment_id_hex: replicaSetId.toString("hex"),
    },
    activation_update: { ...activation, update_id_hex: activationId.toString("hex") },
    revocation_update: { ...revocation, update_id_hex: revocationId.toString("hex") },
    activation_acknowledgement: acknowledgement,
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
  if (!result.emit) { assert(result.schema, "--schema is required"); assert(result.vectors, "--vectors is required"); }
  return result;
}

const defaultInput = {
  owner_secret_key_hex: "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f",
  recipient_secret_key_hex: "303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f",
  owner_account_id_hex: "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
  recipient_account_id_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  owner_encryption_public_key_hex: "505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f",
  scope_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  replica_store_key_2_hex: "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8",
  replica_store_key_3_hex: "29acbae141bccaf0b22e1a94d34d0bc7361e526d0bfe12c89794bc9322966dd7",
  legacy_service_url: "https://mailbox.example.invalid/v1/",
  local_encapsulated_key_hex: "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeef",
  local_ciphertext_hex: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
  offer_encapsulated_key_hex: "e0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
  offer_ciphertext_hex: "2122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40",
  session_binding_hex: "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff000102030405060708090a0b0c0d0e0f",
  created_at: 1800001000,
  expires_at: 1800004600,
  revoked_at: 1800002000,
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
assert(schema.milestone === "M1.1.18" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.cddl.path === "CANONICAL-MAILBOX-PROVISIONING.cddl" &&
  schema.cddl.root_rule === "kilogram-mailbox-provisioning-v1", "schema CDDL binding changed");
equalJson(generate(vectors.input, mailbox), vectors.expected, "canonical mailbox provisioning vectors");

if (options.selfTest) {
  const changedSession = structuredClone(vectors.input);
  changedSession.session_binding_hex = `ff${changedSession.session_binding_hex.slice(2)}`;
  assert(generate(changedSession, mailbox).activation_acknowledgement.signature_hex !==
    vectors.expected.activation_acknowledgement.signature_hex,
  "changed session binding preserved acknowledgement signature");
  const changedMode = structuredClone(vectors.input);
  changedMode.legacy_service_url = "https://other.example.invalid/v1/";
  assert(generate(changedMode, mailbox).legacy_binding_id_hex !== vectors.expected.legacy_binding_id_hex,
    "changed service preserved legacy binding ID");
  process.stdout.write("changed_session_binding=rejected\n");
  process.stdout.write("changed_legacy_service=rejected\n");
  process.stdout.write("canonical_mailbox_provisioning_reference_self_test=passed\n");
}
process.stdout.write("independent_ed25519=node-crypto\n");
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_mailbox_provisioning_reference=verified\n");
