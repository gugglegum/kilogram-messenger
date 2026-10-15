import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  deviceCertificate: "kilogram:device-certificate-signature:cbor-v1\0",
  authoritySnapshot: "kilogram:account-authority-snapshot-signature:cbor-v1\0",
  deviceList: "kilogram:account-device-list-signature:cbor-v1\0",
  acceptedObservation: "kilogram:accepted-endpoint-observation:cbor-v1\0",
  acceptedObservationId: "kilogram:accepted-endpoint-observation-id:cbor-v1\0",
  bundle: "kilogram:endpoint-announcement-bundle:cbor-v1\0",
  bundleId: "kilogram:endpoint-announcement-bundle-id:cbor-v1\0",
  conversation: "kilogram:conversation-label:v1\0",
}).map(([name, value]) => [name, Buffer.from(value)]));
const NESTED_DOMAINS = {
  observation: Buffer.from("kilogram:ticket-publication-observation:cbor-v1\0"),
  proof: Buffer.from("kilogram:ticket-publication-conflict-proof:cbor-v1\0"),
  resolution: Buffer.from("kilogram:publication-conflict-resolution-root-signature:cbor-v1\0"),
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
  return { content, preimage, signature, cbor: array([uint(VERSION), bytes(content), bytes(signature)]) };
}
function signedInline(domain, content, signer) {
  const value = signed(domain, content, signer);
  return { ...value, cbor: array([uint(VERSION), content, bytes(value.signature)]) };
}
function verifyFixtureSignature(expected, domain, publicKey, label) {
  const content = fromHex(expected.content_cbor_hex, undefined, `${label} content`);
  const signature = fromHex(expected.signature_hex, 64, `${label} signature`);
  assert(verifyEd25519(null, Buffer.concat([domain, content]), publicFromBytes(publicKey), signature),
    `${label} nested signature failed`);
  const cbor = array([uint(VERSION), bytes(content), bytes(signature)]);
  assert(cbor.toString("hex") === expected.signed_cbor_hex, `${label} envelope diverged`);
  return cbor;
}
function signedJson(value, objectId) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    object_id_hex: objectId.toString("hex"),
  };
}
function certificate(root, accountId, device, encryptionKey, sequence) {
  const content = array([
    uint(VERSION), bytes(accountId), bytes(device.publicKey), bytes(encryptionKey),
    uint(sequence), array([uint(1), uint(2)]),
  ]);
  return signedInline(DOMAINS.deviceCertificate, content, root);
}
function authorityAndRoster(input, keys) {
  const snapshotContent = array([
    uint(VERSION), bytes(keys.root.publicKey), uint(input.authority_revision), array([]),
  ]);
  const snapshot = signedInline(DOMAINS.authoritySnapshot, snapshotContent, keys.root);
  const certificates = [
    certificate(keys.root, keys.root.publicKey, keys.source,
      fromHex(input.source_encryption_public_key_hex, 32, "source encryption key"), 1),
    certificate(keys.root, keys.root.publicKey, keys.recipient,
      fromHex(input.recipient_encryption_public_key_hex, 32, "recipient encryption key"), 2),
    certificate(keys.root, keys.root.publicKey, keys.detector,
      fromHex(input.detector_encryption_public_key_hex, 32, "detector encryption key"), 3),
  ].sort((left, right) => Buffer.compare(deviceIdFromCertificateContent(left.content),
    deviceIdFromCertificateContent(right.content)));
  const rosterContent = array([
    uint(VERSION), bytes(snapshot.cbor), array(certificates.map((value) => bytes(value.cbor))),
  ]);
  return { snapshot, certificates,
    roster: signedInline(DOMAINS.deviceList, rosterContent, keys.root) };
}
function deviceIdFromCertificateContent(content) {
  // All vector IDs are fixed 32-byte byte strings. The first two fields are
  // version and Account ID: 01 58 20 <32>; Device ID follows as 58 20 <32>.
  const offset = 1 + 1 + 2 + 32 + 2;
  return content.subarray(offset, offset + 32);
}
function state(kind, value = null) {
  return value === null
    ? array([uint(VERSION), uint(kind)])
    : array([uint(VERSION), uint(kind), bytes(value)]);
}
function candidate(peerDeviceId, route, writeKey, ticket, announcementState) {
  return array([
    uint(VERSION), bytes(peerDeviceId), uint(1), uint(route), bytes(writeKey),
    bytes(Buffer.from(ticket, "utf8")), bytes(announcementState),
  ]);
}
function contact(label, peerAccountId, candidateValue) {
  const labelBytes = Buffer.from(label, "utf8");
  const conversationId = domainHash(DOMAINS.conversation, labelBytes);
  return {
    conversationId,
    cbor: array([
      uint(VERSION), bytes(labelBytes), bytes(conversationId), bytes(peerAccountId),
      array([bytes(candidateValue)]),
    ]),
  };
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}

function generate(input, conflict) {
  const keys = {
    root: key(input.local_account_seed_hex, "local Account seed"),
    source: key(input.source_device_seed_hex, "source Device seed"),
    recipient: key(input.recipient_device_seed_hex, "recipient Device seed"),
    detector: key(input.detector_device_seed_hex, "detector Device seed"),
  };
  const expectedKeys = conflict.expected.public_keys;
  assert(keys.root.publicKey.toString("hex") === expectedKeys.local_account_id_hex,
    "local Account key diverged from conflict fixture");
  assert(keys.source.publicKey.toString("hex") === expectedKeys.observer_1_device_id_hex,
    "source Device key diverged from conflict fixture");
  assert(keys.recipient.publicKey.toString("hex") === expectedKeys.observer_2_device_id_hex,
    "recipient Device key diverged from conflict fixture");
  assert(keys.detector.publicKey.toString("hex") === expectedKeys.detector_device_id_hex,
    "detector Device key diverged from conflict fixture");
  const peerAccountId = fromHex(expectedKeys.publisher_account_id_hex, 32, "peer Account ID");
  const peerDeviceId = fromHex(expectedKeys.publisher_device_id_hex, 32, "peer Device ID");
  const oldWriteKey = fromHex(expectedKeys.old_write_key_hex, 32, "old write key");
  const newWriteKey = fromHex(expectedKeys.new_write_key_hex, 32, "new write key");
  const directObservation = verifyFixtureSignature(conflict.expected.first_observation,
    NESTED_DOMAINS.observation, keys.source.publicKey, "direct observation");
  const forwardedObservation = verifyFixtureSignature(conflict.expected.conflicting_observation,
    NESTED_DOMAINS.observation, keys.recipient.publicKey, "forwarded observation");
  const conflictProof = verifyFixtureSignature(conflict.expected.conflict_proof,
    NESTED_DOMAINS.proof, keys.detector.publicKey, "conflict proof");
  const resolution = verifyFixtureSignature(conflict.expected.resolution,
    NESTED_DOMAINS.resolution, keys.root.publicKey, "Root resolution");

  const acceptedContent = array([
    uint(VERSION), bytes(keys.root.publicKey), bytes(keys.source.publicKey),
    bytes(keys.recipient.publicKey),
    bytes(fromHex(input.source_bundle_id_hex, 32, "source bundle ID")),
    uint(input.imported_at_unix_seconds), bytes(forwardedObservation),
  ]);
  const accepted = signed(DOMAINS.acceptedObservation, acceptedContent, keys.source);
  const acceptedId = domainHash(DOMAINS.acceptedObservationId, accepted.cbor);
  const roster = authorityAndRoster(input, keys).roster;
  const contacts = [
    contact("direct-observation", peerAccountId, candidate(peerDeviceId, 0, oldWriteKey,
      conflict.input.first_ticket, state(1, directObservation))),
    contact("accepted-observation", peerAccountId, candidate(peerDeviceId, 0, oldWriteKey,
      conflict.input.conflicting_ticket, state(2, accepted.cbor))),
    contact("publication-conflict", peerAccountId, candidate(peerDeviceId, 0, oldWriteKey,
      conflict.input.first_ticket, state(3, conflictProof))),
    contact("publication-resolution", peerAccountId, candidate(peerDeviceId,
      conflict.input.route_policy, newWriteKey, conflict.input.replacement_ticket,
      state(4, resolution))),
  ].sort((left, right) => Buffer.compare(left.conversationId, right.conversationId));
  const bundleContent = array([
    uint(VERSION), bytes(roster.cbor), bytes(keys.source.publicKey),
    bytes(keys.recipient.publicKey), uint(input.created_at_unix_seconds),
    uint(input.expires_at_unix_seconds), array(contacts.map((value) => bytes(value.cbor))),
  ]);
  const bundle = signed(DOMAINS.bundle, bundleContent, keys.source);
  const bundleId = domainHash(DOMAINS.bundleId, bundle.cbor);
  return {
    accepted_observation: signedJson(accepted, acceptedId),
    signed_bundle: {
      ...signedJson(bundle, bundleId),
      valid_at_unix_seconds: input.created_at_unix_seconds + 1,
    },
  };
}

function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--conflict-vectors", "--output"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["conflict-vectors"], "--conflict-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  local_account_seed_hex: "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
  source_device_seed_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  recipient_device_seed_hex: "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
  detector_device_seed_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  source_encryption_public_key_hex: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
  recipient_encryption_public_key_hex: "2122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40",
  detector_encryption_public_key_hex: "4142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f60",
  source_bundle_id_hex: "5555555555555555555555555555555555555555555555555555555555555555",
  authority_revision: 8,
  imported_at_unix_seconds: 1800000045,
  created_at_unix_seconds: 1800000050,
  expires_at_unix_seconds: 1800000950,
};

const options = parse(process.argv.slice(2));
const conflict = JSON.parse(readFileSync(options["conflict-vectors"], "utf8"));
if (options.emit) {
  const generated = `${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    dependencies: ["CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"],
    expected: generate(defaultInput, conflict),
  }, null, 2)}\n`;
  if (options.output) {
    writeFileSync(options.output, generated, "utf8");
    process.stdout.write(`canonical_endpoint_announcement_vectors_written=${options.output}\n`);
  } else process.stdout.write(generated);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.23" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "endpoint-announcement schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "endpoint-announcement vector header changed");
equalJson(generate(vectors.input, conflict), vectors.expected,
  "independent endpoint-announcement vector");
if (options.selfTest) {
  for (const [label, mutate] of [
    ["changed_bundle_signature", (value) => { value.expected.signed_bundle.signature_hex =
      `00${value.expected.signed_bundle.signature_hex.slice(2)}`; }],
    ["changed_accepted_id", (value) => { value.expected.accepted_observation.object_id_hex =
      `00${value.expected.accepted_observation.object_id_hex.slice(2)}`; }],
    ["changed_recipient", (value) => { value.input.recipient_device_seed_hex =
      "818182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f"; }],
  ]) {
    const changed = JSON.parse(JSON.stringify(vectors)); mutate(changed);
    let rejected = false;
    try { equalJson(generate(changed.input, conflict), changed.expected, label); } catch { rejected = true; }
    assert(rejected, `${label} was accepted`);
  }
  process.stdout.write("canonical_endpoint_announcement_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_endpoint_announcement_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519+javascript-blake3\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("hpke_envelope_covered=false\n");
  process.stdout.write("active_wire_changed=false\n");
}
