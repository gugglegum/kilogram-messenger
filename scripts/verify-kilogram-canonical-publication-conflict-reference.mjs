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
const CLAIM_URI_PREFIX = "kilogram://publication-conflict/cbor-v1/";
const DOMAINS = Object.fromEntries(Object.entries({
  publicationSignature: "kilogram:ticket-publication:cbor-v1\0",
  publicationId: "kilogram:ticket-publication-id:cbor-v1\0",
  ticketDigest: "kilogram:ticket-publication-ticket-digest:v1\0",
  channelId: "kilogram:ticket-publication-capability-channel:v1\0",
  observationSignature: "kilogram:ticket-publication-observation:cbor-v1\0",
  observationId: "kilogram:ticket-publication-observation-id:cbor-v1\0",
  proofSignature: "kilogram:ticket-publication-conflict-proof:cbor-v1\0",
  proofId: "kilogram:ticket-publication-conflict-proof-id:cbor-v1\0",
  evidenceId: "kilogram:ticket-publication-conflict-evidence-id:cbor-v1\0",
  requestSignature: "kilogram:publication-conflict-resolution-request:cbor-v1\0",
  requestId: "kilogram:publication-conflict-resolution-request-id:cbor-v1\0",
  resolutionSignature: "kilogram:publication-conflict-resolution-root-signature:cbor-v1\0",
  resolutionId: "kilogram:publication-conflict-resolution-id:cbor-v1\0",
  replacementDigest: "kilogram:publication-conflict-replacement-ticket-digest:cbor-v1\0",
  confirmation: "kilogram:publication-conflict-confirmation:cbor-v1\0",
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
function resolutionRequest(input, conflictProof) {
  const replacementTicket = Buffer.from(input.raw.replacement_ticket, "utf8");
  const content = array([
    uint(VERSION), bytes(input.localAccountId), bytes(input.detector.publicKey),
    uint(input.raw.authority_revision), bytes(conflictProof.cbor),
    bytes(input.publisherAccountId), bytes(input.publisher.publicKey),
    bytes(input.oldWrite.publicKey), bytes(input.newWrite.publicKey),
    uint(input.raw.old_channel_epoch), uint(input.raw.new_channel_epoch),
    uint(input.raw.route_policy), bytes(replacementTicket),
    uint(input.raw.requested_at_unix_seconds),
  ]);
  const value = signed(DOMAINS.requestSignature, content, input.detector);
  return {
    ...value,
    id: domainHash(DOMAINS.requestId, value.cbor),
    replacementTicket,
    replacementDigest: domainHash(DOMAINS.replacementDigest, replacementTicket),
  };
}
function resolution(input, conflictProof, request) {
  const content = array([
    uint(VERSION), bytes(request.id), bytes(input.localAccountId),
    uint(input.raw.authority_revision), bytes(conflictProof.evidenceId),
    bytes(input.publisherAccountId), bytes(input.publisher.publicKey),
    bytes(input.oldWrite.publicKey), bytes(input.newWrite.publicKey),
    uint(input.raw.old_channel_epoch), uint(input.raw.new_channel_epoch),
    uint(input.raw.route_policy), bytes(request.replacementDigest),
    uint(input.raw.authorized_at_unix_seconds),
  ]);
  const value = signed(DOMAINS.resolutionSignature, content, input.localAccount);
  return { ...value, id: domainHash(DOMAINS.resolutionId, value.cbor) };
}
function resolutionResponse(request, resolutionValue) {
  return array([
    uint(VERSION), bytes(request.id), bytes(resolutionValue.cbor), bytes(request.replacementTicket),
  ]);
}
function confirmationCode(common) {
  const u64 = (value) => { const output = Buffer.alloc(8); output.writeBigUInt64BE(BigInt(value)); return output; };
  return domainHash(DOMAINS.confirmation, lengthDelimited([
    common.requestId, common.localAccountId, u64(common.authorityRevision),
    common.evidenceId, common.peerAccountId, common.peerDeviceId,
    common.oldChannelId, common.newChannelId, u64(common.oldEpoch),
    u64(common.newEpoch), u64(common.routePolicy), common.replacementDigest,
  ])).subarray(0, 12);
}
function claim(input, conflictProof, request, resolutionValue, kind) {
  const common = {
    requestId: request.id,
    localAccountId: input.localAccountId,
    authorityRevision: input.raw.authority_revision,
    evidenceId: conflictProof.evidenceId,
    peerAccountId: input.publisherAccountId,
    peerDeviceId: input.publisher.publicKey,
    oldChannelId: domainHash(DOMAINS.channelId, input.oldWrite.publicKey),
    newChannelId: domainHash(DOMAINS.channelId, input.newWrite.publicKey),
    oldEpoch: input.raw.old_channel_epoch,
    newEpoch: input.raw.new_channel_epoch,
    routePolicy: input.raw.route_policy,
    replacementDigest: request.replacementDigest,
  };
  const artifactId = kind === 0 ? request.id : resolutionValue.id;
  const confirmation = confirmationCode(common);
  const cbor = array([
    uint(VERSION), uint(kind), bytes(artifactId), bytes(common.requestId),
    bytes(common.localAccountId), uint(common.authorityRevision), bytes(common.evidenceId),
    bytes(common.peerAccountId), bytes(common.peerDeviceId), bytes(common.oldChannelId),
    bytes(common.newChannelId), uint(common.oldEpoch), uint(common.newEpoch),
    uint(common.routePolicy), bytes(common.replacementDigest), bytes(confirmation),
  ]);
  return {
    cbor,
    artifactId,
    confirmation,
    uri: `${CLAIM_URI_PREFIX}${cbor.toString("base64url")}`,
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
function claimJson(value) {
  return {
    cbor_hex: value.cbor.toString("hex"),
    uri: value.uri,
    artifact_id_hex: value.artifactId.toString("hex"),
    confirmation_code_hex: value.confirmation.toString("hex"),
    confirmation_code: `KPC1-${value.confirmation.toString("hex").toUpperCase().match(/.{4}/g).join("-")}`,
  };
}
function generate(raw) {
  const input = {
    raw,
    publisher: key(fromHex(raw.publisher_device_seed_hex, 32, "publisher seed")),
    observer1: key(fromHex(raw.observer_1_seed_hex, 32, "observer 1 seed")),
    observer2: key(fromHex(raw.observer_2_seed_hex, 32, "observer 2 seed")),
    detector: key(fromHex(raw.detector_seed_hex, 32, "detector seed")),
    publisherAccountId: key(fromHex(raw.publisher_account_seed_hex, 32,
      "publisher Account seed")).publicKey,
    localAccount: key(fromHex(raw.local_account_seed_hex, 32, "local Account seed")),
    oldWrite: key(fromHex(raw.old_write_seed_hex, 32, "old write seed")),
    newWrite: key(fromHex(raw.new_write_seed_hex, 32, "new write seed")),
    publishedAt: raw.published_at_unix_seconds,
    expiresAt: raw.expires_at_unix_seconds,
    observedAt: raw.observed_at_unix_seconds,
    detectedAt: raw.detected_at_unix_seconds,
  };
  input.localAccountId = input.localAccount.publicKey;
  input.channelId = domainHash(DOMAINS.channelId, input.oldWrite.publicKey);
  assert(input.channelId.equals(fromHex(raw.channel_id_hex, 32, "channel ID")),
    "channel ID does not match the old write key");
  const first = publication(input, input.publisher, raw.first_ticket);
  const conflicting = publication(input, input.publisher, raw.conflicting_ticket);
  const firstObservation = observation(input, input.observer1, first);
  const conflictingObservation = observation(input, input.observer2, conflicting);
  const conflictProof = proof(input, input.detector, firstObservation, conflictingObservation);
  const request = resolutionRequest(input, conflictProof);
  const resolutionValue = resolution(input, conflictProof, request);
  const response = resolutionResponse(request, resolutionValue);
  const requestClaim = claim(input, conflictProof, request, resolutionValue, 0);
  const responseClaim = claim(input, conflictProof, request, resolutionValue, 1);
  assert(requestClaim.confirmation.equals(responseClaim.confirmation),
    "request and response confirmation codes diverged");
  return {
    public_keys: {
      publisher_account_id_hex: input.publisherAccountId.toString("hex"),
      local_account_id_hex: input.localAccountId.toString("hex"),
      publisher_device_id_hex: input.publisher.publicKey.toString("hex"),
      observer_1_device_id_hex: input.observer1.publicKey.toString("hex"),
      observer_2_device_id_hex: input.observer2.publicKey.toString("hex"),
      detector_device_id_hex: input.detector.publicKey.toString("hex"),
      old_write_key_hex: input.oldWrite.publicKey.toString("hex"),
      new_write_key_hex: input.newWrite.publicKey.toString("hex"),
      old_channel_id_hex: input.channelId.toString("hex"),
      new_channel_id_hex: domainHash(DOMAINS.channelId, input.newWrite.publicKey).toString("hex"),
    },
    first_publication: signedJson(first, {
      publication_id_hex: first.id.toString("hex"), ticket_digest_hex: first.digest.toString("hex"),
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
    resolution_request: signedJson(request, {
      request_id_hex: request.id.toString("hex"),
      replacement_ticket_digest_hex: request.replacementDigest.toString("hex"),
    }),
    resolution: signedJson(resolutionValue, {
      resolution_id_hex: resolutionValue.id.toString("hex"),
      replacement_ticket_digest_hex: request.replacementDigest.toString("hex"),
    }),
    resolution_response: { cbor_hex: response.toString("hex") },
    request_claim: claimJson(requestClaim),
    response_claim: claimJson(responseClaim),
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
    else if (["--schema", "--vectors", "--output"].includes(argument)) {
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
  old_write_seed_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  new_write_seed_hex: "e0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
  channel_id_hex: "d97473e45f9ad2e4eed319b4c4369d6056505abf401864b06c667cb12507f032",
  first_ticket: "kilogram-ticket-one",
  conflicting_ticket: "kilogram-ticket-two",
  replacement_ticket: "kilogram-ticket-replacement",
  authority_revision: 8,
  old_channel_epoch: 0,
  new_channel_epoch: 1,
  route_policy: 2,
  published_at_unix_seconds: 1800000000,
  expires_at_unix_seconds: 1800000900,
  observed_at_unix_seconds: 1800000010,
  detected_at_unix_seconds: 1800000020,
  requested_at_unix_seconds: 1800000030,
  authorized_at_unix_seconds: 1800000040,
};

const options = parse(process.argv.slice(2));
if (options.emit) {
  const generated = `${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(defaultInput),
  }, null, 2)}\n`;
  if (options.output) {
    writeFileSync(options.output, generated, "utf8");
    process.stdout.write(`canonical_publication_conflict_vectors_written=${options.output}\n`);
  } else {
    process.stdout.write(generated);
  }
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.22" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.coverage.status === "complete" && schema.coverage.remaining.length === 0,
  "publication-conflict coverage boundary changed");
equalJson(generate(vectors.input), vectors.expected, "canonical publication conflict vectors");

if (options.selfTest) {
  const changedTicket = structuredClone(vectors.input);
  changedTicket.replacement_ticket = `${changedTicket.replacement_ticket}!`;
  const changedTicketOutput = generate(changedTicket);
  assert(changedTicketOutput.resolution_request.request_id_hex !==
    vectors.expected.resolution_request.request_id_hex, "changed ticket preserved request ID");
  assert(changedTicketOutput.request_claim.confirmation_code_hex !==
    vectors.expected.request_claim.confirmation_code_hex, "changed ticket preserved claim code");
  const changedRoot = structuredClone(vectors.input);
  changedRoot.local_account_seed_hex = vectors.input.publisher_account_seed_hex;
  assert(generate(changedRoot).resolution.signature_hex !== vectors.expected.resolution.signature_hex,
    "changed Root preserved resolution signature");
  assert(vectors.expected.request_claim.artifact_id_hex !==
    vectors.expected.response_claim.artifact_id_hex, "cross-kind artifact IDs coincide");
  process.stdout.write("changed_replacement_ticket=rejected\n");
  process.stdout.write("changed_root=rejected\n");
  process.stdout.write("cross_kind_claim=rejected\n");
  process.stdout.write("canonical_publication_conflict_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("independent_ed25519=verified\n");
process.stdout.write("canonical_publication_conflict_reference=verified\n");
