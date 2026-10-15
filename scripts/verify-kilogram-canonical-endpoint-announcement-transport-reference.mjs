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
const BUNDLE_SIGNATURE_DOMAIN = Buffer.from(
  "kilogram:endpoint-announcement-bundle:cbor-v1\0",
);
const BUNDLE_ID_DOMAIN = Buffer.from(
  "kilogram:endpoint-announcement-bundle-id:cbor-v1\0",
);
const ENVELOPE_HPKE_INFO = Buffer.from(
  "kilogram:endpoint-announcement-envelope:cbor-v1\0",
);
const ACK_SIGNATURE_DOMAIN = Buffer.from(
  "kilogram:endpoint-announcement-acknowledgement:cbor-v1\0",
);
const TRANSPORT_BINDING_DOMAIN = Buffer.from(
  "kilogram:endpoint-announcement-transport-vector:cbor-v1\0",
);
const DEFAULT_SESSION = Buffer.from(
  "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeef",
  "hex",
);
const DEFAULT_APPLIED = {
  contact_added_count: 3,
  endpoint_added_count: 2,
  publication_binding_added_count: 2,
  observation_evidence_added_count: 1,
  publication_conflict_added_count: 1,
  publication_resolution_added_count: 0,
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
function hex(value) { return value.toString("hex"); }
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}

function argument(buffer, state, additional) {
  function take(count) {
    assert(state.offset + count <= buffer.length, "truncated CBOR argument");
    const start = state.offset; state.offset += count; return buffer.subarray(start, state.offset);
  }
  if (additional < 24) return additional;
  if (additional === 24) {
    const value = take(1)[0]; assert(value >= 24, "non-minimal CBOR argument"); return value;
  }
  if (additional === 25) {
    const value = take(2).readUInt16BE(); assert(value > 0xff, "non-minimal CBOR argument"); return value;
  }
  if (additional === 26) {
    const value = take(4).readUInt32BE(); assert(value > 0xffff, "non-minimal CBOR argument"); return value;
  }
  if (additional === 27) {
    const value = take(8).readBigUInt64BE();
    assert(value > 0xffffffffn && value <= BigInt(Number.MAX_SAFE_INTEGER), "unsupported CBOR integer");
    return Number(value);
  }
  return fail("indefinite or reserved CBOR argument");
}
function decodeItem(buffer, state) {
  assert(state.offset < buffer.length, "truncated CBOR item");
  const initial = buffer[state.offset++];
  const major = initial >> 5;
  const length = argument(buffer, state, initial & 31);
  if (major === 0) return length;
  if (major === 2) {
    assert(state.offset + length <= buffer.length, "truncated CBOR byte string");
    const value = buffer.subarray(state.offset, state.offset + length); state.offset += length; return value;
  }
  if (major === 4) {
    const value = [];
    for (let index = 0; index < length; index += 1) value.push(decodeItem(buffer, state));
    return value;
  }
  return fail(`unsupported CBOR major type ${major}`);
}
function decodeExact(buffer) {
  const state = { offset: 0 };
  const value = decodeItem(buffer, state);
  assert(state.offset === buffer.length, "trailing CBOR bytes");
  return value;
}
function expectArray(value, length, label) {
  assert(Array.isArray(value) && value.length === length, `${label} shape changed`);
  assert(value[0] === VERSION, `${label} version changed`);
  return value;
}
function expectBytes(value, length, label) {
  assert(Buffer.isBuffer(value) && (length === undefined || value.length === length),
    `${label} byte string changed`);
  return value;
}
function parseBundle(endpointVectors) {
  const bundleBytes = fromHex(
    endpointVectors.expected.signed_bundle.signed_cbor_hex, undefined, "signed bundle",
  );
  const signedBundle = expectArray(decodeExact(bundleBytes), 3, "signed bundle");
  const contentBytes = expectBytes(signedBundle[1], undefined, "bundle content");
  const signature = expectBytes(signedBundle[2], 64, "bundle signature");
  const content = expectArray(decodeExact(contentBytes), 7, "bundle content");
  const sourceDeviceId = expectBytes(content[2], 32, "source Device ID");
  const recipientDeviceId = expectBytes(content[3], 32, "recipient Device ID");
  const contacts = content[6];
  assert(Array.isArray(contacts), "bundle contacts changed");
  const statistics = {
    authority_revision: endpointVectors.input.authority_revision,
    contact_count: contacts.length,
    endpoint_count: 0,
    observation_evidence_count: 0,
    publication_conflict_count: 0,
    publication_resolution_count: 0,
  };
  for (const contactBytes of contacts) {
    const contact = expectArray(decodeExact(expectBytes(contactBytes, undefined, "contact")), 5, "contact");
    assert(Array.isArray(contact[4]), "contact endpoint list changed");
    statistics.endpoint_count += contact[4].length;
    for (const endpointBytes of contact[4]) {
      const endpoint = expectArray(
        decodeExact(expectBytes(endpointBytes, undefined, "endpoint")), 7, "endpoint",
      );
      const state = decodeExact(expectBytes(endpoint[6], undefined, "endpoint state"));
      assert(Array.isArray(state) && (state.length === 2 || state.length === 3) &&
        state[0] === VERSION, "endpoint state shape changed");
      if (state[1] === 1 || state[1] === 2) statistics.observation_evidence_count += 1;
      else if (state[1] === 3) statistics.publication_conflict_count += 1;
      else if (state[1] === 4) statistics.publication_resolution_count += 1;
      else assert(state[1] === 0, "endpoint state registry changed");
    }
  }
  return { bundleBytes, contentBytes, signature, sourceDeviceId, recipientDeviceId, statistics };
}

function generate(endpointVectors, transportInput) {
  assert(endpointVectors.schema_version === VERSION &&
    endpointVectors.profile === "kilogram-deterministic-cbor-v1", "endpoint dependency header changed");
  const source = key(endpointVectors.input.source_device_seed_hex, "source Device seed");
  const recipient = key(endpointVectors.input.recipient_device_seed_hex, "recipient Device seed");
  const account = key(endpointVectors.input.local_account_seed_hex, "Account seed");
  const parsed = parseBundle(endpointVectors);
  assert(parsed.sourceDeviceId.equals(source.publicKey), "bundle source Device changed");
  assert(parsed.recipientDeviceId.equals(recipient.publicKey), "bundle recipient Device changed");
  assert(verifyEd25519(null, Buffer.concat([BUNDLE_SIGNATURE_DOMAIN, parsed.contentBytes]),
    publicFromBytes(source.publicKey), parsed.signature), "bundle signature failed independently");
  const bundleId = domainHash(BUNDLE_ID_DOMAIN, parsed.bundleBytes);
  assert(hex(bundleId) === endpointVectors.expected.signed_bundle.object_id_hex,
    "bundle ID failed independently");

  const sessionBinding = fromHex(transportInput.session_binding_hex, 32, "session binding");
  const applied = transportInput.applied_counts;
  for (const [name, value] of Object.entries(applied)) {
    assert(Number.isSafeInteger(value) && value >= 0, `${name} is invalid`);
  }
  assert(applied.contact_added_count <= parsed.statistics.contact_count, "contact added count exceeds total");
  assert(applied.endpoint_added_count <= parsed.statistics.endpoint_count, "endpoint added count exceeds total");
  assert(applied.publication_binding_added_count <= parsed.statistics.endpoint_count,
    "publication binding added count exceeds total");
  assert(applied.observation_evidence_added_count <= parsed.statistics.observation_evidence_count,
    "observation added count exceeds total");
  assert(applied.publication_conflict_added_count <= parsed.statistics.publication_conflict_count,
    "conflict added count exceeds total");
  assert(applied.publication_resolution_added_count <= parsed.statistics.publication_resolution_count,
    "resolution added count exceeds total");

  const header = array([
    uint(VERSION), bytes(account.publicKey), bytes(source.publicKey), bytes(recipient.publicKey), bytes(bundleId),
  ]);
  const encapsulatedKey = fromHex(transportInput.fixed_hpke_seal.encapsulated_key_hex, 32,
    "HPKE encapsulated key");
  const ciphertext = fromHex(transportInput.fixed_hpke_seal.ciphertext_hex, undefined, "HPKE ciphertext");
  assert(ciphertext.length > 0 && ciphertext.length <= 16777232, "HPKE ciphertext bound changed");
  const envelope = array([
    uint(VERSION), bytes(header), bytes(encapsulatedKey), bytes(ciphertext),
  ]);

  const stats = parsed.statistics;
  const acknowledgementContent = array([
    uint(VERSION), bytes(sessionBinding), bytes(bundleId), bytes(source.publicKey), bytes(recipient.publicKey),
    uint(stats.authority_revision), uint(stats.contact_count), uint(applied.contact_added_count),
    uint(stats.endpoint_count), uint(applied.endpoint_added_count),
    uint(applied.publication_binding_added_count), uint(stats.observation_evidence_count),
    uint(applied.observation_evidence_added_count), uint(stats.publication_conflict_count),
    uint(applied.publication_conflict_added_count), uint(stats.publication_resolution_count),
    uint(applied.publication_resolution_added_count),
  ]);
  const acknowledgement = signed(ACK_SIGNATURE_DOMAIN, acknowledgementContent, recipient);
  const bindingInput = array([bytes(header), bytes(envelope), bytes(acknowledgement.cbor)]);
  return {
    domains: {
      hpke_info_hex: hex(ENVELOPE_HPKE_INFO),
      acknowledgement_signature_domain_hex: hex(ACK_SIGNATURE_DOMAIN),
      transport_binding_domain_hex: hex(TRANSPORT_BINDING_DOMAIN),
    },
    bundle_statistics: stats,
    envelope_header: { cbor_hex: hex(header), aad_hex: hex(header) },
    encrypted_bundle: {
      encapsulated_key_hex: hex(encapsulatedKey),
      ciphertext_hex: hex(ciphertext),
      cbor_hex: hex(envelope),
    },
    acknowledgement: {
      content_cbor_hex: hex(acknowledgement.content),
      signing_preimage_hex: hex(acknowledgement.preimage),
      signature_hex: hex(acknowledgement.signature),
      signed_cbor_hex: hex(acknowledgement.cbor),
    },
    transport_binding_hash_hex: hex(domainHash(TRANSPORT_BINDING_DOMAIN, bindingInput)),
  };
}

function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--endpoint-vectors", "--seal-input", "--output"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["endpoint-vectors"], "--endpoint-vectors is required");
  if (result.emit) {
    assert(result["seal-input"] && result.output, "--seal-input and --output are required with --emit");
  } else {
    assert(result.schema && result.vectors, "--schema and --vectors are required");
  }
  return result;
}

const options = parse(process.argv.slice(2));
const endpointVectors = JSON.parse(readFileSync(options["endpoint-vectors"], "utf8"));
if (options.emit) {
  const fixedSeal = JSON.parse(readFileSync(options["seal-input"], "utf8"));
  const input = {
    session_binding_hex: hex(DEFAULT_SESSION),
    applied_counts: DEFAULT_APPLIED,
    fixed_hpke_seal: {
      encapsulated_key_hex: fixedSeal.encapsulated_key_hex,
      ciphertext_hex: fixedSeal.ciphertext_hex,
    },
  };
  const document = {
    schema_version: VERSION,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependency: "CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json",
    input,
    expected: generate(endpointVectors, input),
  };
  writeFileSync(options.output, `${JSON.stringify(document, null, 2)}\n`);
  process.stdout.write(`canonical_endpoint_announcement_transport_vectors=${options.output}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.24" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false && schema.completes_surface === "endpoint-announcement",
"schema activation boundary changed");
assert(schema.cddl.path === "CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT.cddl" &&
  schema.cddl.root_rule === "kilogram-endpoint-announcement-transport-v1", "schema CDDL binding changed");
assert(schema.activation.current_runtime_encoding === "postcard-v4-bundle-hpke-v1-envelope-ack-v2" &&
  schema.activation.current_runtime_changed === false &&
  schema.activation.candidate_protocol_identifier === "unassigned", "runtime activation changed");
assert(vectors.dependency === "CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json", "vector dependency changed");
const expected = generate(endpointVectors, vectors.input);
equalJson(expected, vectors.expected, "canonical endpoint-announcement transport vectors");

if (options.selfTest) {
  const ack = vectors.expected.acknowledgement;
  const changedSignature = Buffer.from(ack.signature_hex, "hex"); changedSignature[0] ^= 1;
  assert(!verifyEd25519(null, Buffer.from(ack.signing_preimage_hex, "hex"),
    publicFromBytes(key(endpointVectors.input.recipient_device_seed_hex, "recipient seed").publicKey),
    changedSignature), "changed acknowledgement signature was accepted");
  const changedEnvelope = structuredClone(vectors.expected);
  changedEnvelope.encrypted_bundle.ciphertext_hex =
    `ff${changedEnvelope.encrypted_bundle.ciphertext_hex.slice(2)}`;
  assert(JSON.stringify(changedEnvelope) !== JSON.stringify(expected), "changed ciphertext was accepted");
  const changedSession = structuredClone(vectors.input);
  changedSession.session_binding_hex = `ff${changedSession.session_binding_hex.slice(2)}`;
  assert(generate(endpointVectors, changedSession).acknowledgement.signature_hex !==
    expected.acknowledgement.signature_hex, "changed session binding preserved acknowledgement");
  process.stdout.write("changed_acknowledgement_signature=rejected\n");
  process.stdout.write("changed_envelope_ciphertext=rejected\n");
  process.stdout.write("changed_session_binding=rejected\n");
  process.stdout.write("canonical_endpoint_announcement_transport_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_endpoint_announcement_transport_reference=verified\n");
