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
const DOMAINS = Object.fromEntries(Object.entries({
  publicationSignature: "kilogram:ticket-publication:cbor-v1\0",
  publicationId: "kilogram:ticket-publication-id:cbor-v1\0",
  ticketDigest: "kilogram:ticket-publication-ticket-digest:v1\0",
  observationSignature: "kilogram:ticket-publication-observation:cbor-v1\0",
  observationId: "kilogram:ticket-publication-observation-id:cbor-v1\0",
  proofSignature: "kilogram:ticket-publication-conflict-proof:cbor-v1\0",
  proofId: "kilogram:ticket-publication-conflict-proof-id:cbor-v1\0",
  evidenceId: "kilogram:ticket-publication-conflict-evidence-id:cbor-v1\0",
}).map(([key, value]) => [key, Buffer.from(value)]));

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
const optionalId = (value) => value === null ? array([]) : array([bytes(value)]);
function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function key(seed) {
  const privateKey = createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8",
  });
  const der = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(der.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI");
  return { privateKey, publicKey: der.subarray(SPKI_PREFIX.length) };
}
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function signed(domain, content, signer) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer.privateKey);
  const publicKey = createPublicKey({
    key: Buffer.concat([SPKI_PREFIX, signer.publicKey]), format: "der", type: "spki",
  });
  assert(verifyEd25519(null, preimage, publicKey, signature), "Ed25519 verification failed");
  return { content, preimage, signature, cbor: array([uint(VERSION), bytes(content), bytes(signature)]) };
}
function publication(input, signer, ticket) {
  const ticketBytes = Buffer.from(ticket, "utf8");
  const digest = domainHash(DOMAINS.ticketDigest, ticketBytes);
  const content = array([
    uint(VERSION), bytes(input.channelId), bytes(input.publisherAccountId),
    bytes(signer.publicKey), bytes(input.localAccountId), uint(1), optionalId(null),
    uint(input.publishedAt), uint(input.expiresAt), bytes(digest), bytes(ticketBytes),
  ]);
  const value = signed(DOMAINS.publicationSignature, content, signer);
  return { ...value, digest, id: domainHash(DOMAINS.publicationId, value.cbor) };
}
function observation(input, signer, publicationValue) {
  const content = array([
    uint(VERSION), bytes(input.localAccountId), bytes(signer.publicKey), bytes(input.channelId),
    bytes(input.publisherAccountId), bytes(input.publisher.publicKey), uint(1), optionalId(null),
    uint(1), bytes(publicationValue.id), bytes(publicationValue.digest), uint(input.observedAt),
  ]);
  const value = signed(DOMAINS.observationSignature, content, signer);
  return { ...value, id: domainHash(DOMAINS.observationId, value.cbor) };
}
function lengthDelimited(values) {
  const parts = [];
  for (const value of values) {
    const length = Buffer.alloc(8); length.writeBigUInt64BE(BigInt(value.length));
    parts.push(length, value);
  }
  return Buffer.concat(parts);
}
function proof(input, signer, left, right) {
  const observations = [left, right].sort((a, b) => Buffer.compare(a.id, b.id));
  assert(Buffer.compare(observations[0].id, observations[1].id) < 0,
    "observations are not distinct");
  const content = array([
    uint(VERSION), bytes(input.localAccountId), bytes(signer.publicKey), uint(input.detectedAt),
    bytes(input.channelId), uint(1), bytes(observations[0].cbor), bytes(observations[1].cbor),
  ]);
  const value = signed(DOMAINS.proofSignature, content, signer);
  return {
    ...value,
    id: domainHash(DOMAINS.proofId, value.cbor),
    evidenceId: domainHash(DOMAINS.evidenceId,
      lengthDelimited(observations.map((item) => item.cbor))),
    observations,
  };
}
function signedJson(value, extra = {}) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    ...extra,
  };
}
function generate(raw) {
  const input = {
    publisher: key(fromHex(raw.publisher_device_seed_hex, 32, "publisher seed")),
    observer1: key(fromHex(raw.observer_1_seed_hex, 32, "observer 1 seed")),
    observer2: key(fromHex(raw.observer_2_seed_hex, 32, "observer 2 seed")),
    detector: key(fromHex(raw.detector_seed_hex, 32, "detector seed")),
    publisherAccountId: key(fromHex(raw.publisher_account_seed_hex, 32,
      "publisher Account seed")).publicKey,
    localAccountId: key(fromHex(raw.local_account_seed_hex, 32,
      "local Account seed")).publicKey,
    channelId: fromHex(raw.channel_id_hex, 32, "channel ID"),
    publishedAt: raw.published_at_unix_seconds,
    expiresAt: raw.expires_at_unix_seconds,
    observedAt: raw.observed_at_unix_seconds,
    detectedAt: raw.detected_at_unix_seconds,
  };
  const first = publication(input, input.publisher, raw.first_ticket);
  const conflicting = publication(input, input.publisher, raw.conflicting_ticket);
  const firstObservation = observation(input, input.observer1, first);
  const conflictingObservation = observation(input, input.observer2, conflicting);
  const conflictProof = proof(input, input.detector, firstObservation, conflictingObservation);
  return {
    public_keys: {
      publisher_account_id_hex: input.publisherAccountId.toString("hex"),
      local_account_id_hex: input.localAccountId.toString("hex"),
      publisher_device_id_hex: input.publisher.publicKey.toString("hex"),
      observer_1_device_id_hex: input.observer1.publicKey.toString("hex"),
      observer_2_device_id_hex: input.observer2.publicKey.toString("hex"),
      detector_device_id_hex: input.detector.publicKey.toString("hex"),
    },
    first_publication: signedJson(first, {
      publication_id_hex: first.id.toString("hex"),
      ticket_digest_hex: first.digest.toString("hex"),
    }),
    conflicting_publication: signedJson(conflicting, {
      publication_id_hex: conflicting.id.toString("hex"),
      ticket_digest_hex: conflicting.digest.toString("hex"),
    }),
    first_observation: signedJson(firstObservation, {
      observation_id_hex: firstObservation.id.toString("hex"),
    }),
    conflicting_observation: signedJson(conflictingObservation, {
      observation_id_hex: conflictingObservation.id.toString("hex"),
    }),
    conflict_proof: signedJson(conflictProof, {
      proof_id_hex: conflictProof.id.toString("hex"),
      evidence_id_hex: conflictProof.evidenceId.toString("hex"),
      first_observation_id_hex: conflictProof.observations[0].id.toString("hex"),
      conflicting_observation_id_hex: conflictProof.observations[1].id.toString("hex"),
    }),
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
  publisher_account_seed_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  local_account_seed_hex: "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
  publisher_device_seed_hex: "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
  observer_1_seed_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  observer_2_seed_hex: "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
  detector_seed_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  channel_id_hex: "c6777e544e6df9237103169ebe76d9c5b53e3c64b52c89e5c035bd555382aa21",
  first_ticket: "kilogram-ticket-one",
  conflicting_ticket: "kilogram-ticket-two",
  published_at_unix_seconds: 1800000000,
  expires_at_unix_seconds: 1800000900,
  observed_at_unix_seconds: 1800000010,
  detected_at_unix_seconds: 1800000020,
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
assert(schema.milestone === "M1.1.21" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.coverage.status === "partial" && schema.coverage.remaining.length === 4,
  "publication-conflict coverage boundary changed");
equalJson(generate(vectors.input), vectors.expected, "canonical publication conflict vectors");

if (options.selfTest) {
  const changedTicket = structuredClone(vectors.input);
  changedTicket.first_ticket = `${changedTicket.first_ticket}!`;
  assert(generate(changedTicket).first_publication.publication_id_hex !==
    vectors.expected.first_publication.publication_id_hex, "changed ticket preserved publication ID");
  const changedDetector = structuredClone(vectors.input);
  changedDetector.detector_seed_hex = vectors.input.observer_1_seed_hex;
  assert(generate(changedDetector).conflict_proof.signature_hex !==
    vectors.expected.conflict_proof.signature_hex, "changed detector preserved proof signature");
  process.stdout.write("changed_ticket=rejected\n");
  process.stdout.write("changed_detector=rejected\n");
  process.stdout.write("canonical_publication_conflict_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("independent_ed25519=verified\n");
process.stdout.write("canonical_publication_conflict_reference=verified\n");
