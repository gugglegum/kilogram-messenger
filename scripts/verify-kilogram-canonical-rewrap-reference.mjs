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
  membership: "kilogram:conversation-membership-signature:cbor-v1\0",
  eventSignature: "kilogram:event-signature:cbor-v1\0",
  eventId: "kilogram:event-id:cbor-v1\0",
  sas: "kilogram:history-rewrap-sas:cbor-v1\0",
  inventory: "kilogram:history-rewrap-inventory:cbor-v1\0",
  request: "kilogram:history-rewrap-request-signature:cbor-v1\0",
  manifestId: "kilogram:history-rewrap-manifest-id:cbor-v1\0",
  hpkeInfo: "kilogram:history-rewrap-entry-hpke:cbor-v1\0",
  entryAad: "kilogram:history-rewrap-entry-aad:cbor-v1\0",
  entrySignature: "kilogram:history-rewrap-entry-signature:cbor-v1\0",
  bundleId: "kilogram:history-rewrap-bundle-id:cbor-v1\0",
  transfer: "kilogram:history-rewrap-transfer-signature:cbor-v1\0",
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
function signed(domain, content, signer, inline = false) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer.privateKey);
  assert(verifyEd25519(null, preimage, publicFromBytes(signer.publicKey), signature),
    "generated Ed25519 signature failed");
  const envelopeContent = inline ? content : bytes(content);
  return {
    content, preimage, signature,
    cbor: array([uint(VERSION), envelopeContent, bytes(signature)]),
  };
}
function signedJson(value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
  };
}
function certificate(root, accountId, device, encryptionKey, sequence) {
  const content = array([
    uint(VERSION), bytes(accountId), bytes(device.publicKey), bytes(encryptionKey),
    uint(sequence), array([uint(1), uint(2)]),
  ]);
  return signed(DOMAINS.deviceCertificate, content, root, true);
}
function deviceIdFromCertificateContent(content) {
  const offset = 1 + 1 + 2 + 32 + 2;
  return content.subarray(offset, offset + 32);
}
function buildRoster(endpoint) {
  const input = endpoint.input;
  const keys = {
    root: key(input.local_account_seed_hex, "Account seed"),
    source: key(input.source_device_seed_hex, "source Device seed"),
    recipient: key(input.recipient_device_seed_hex, "recipient Device seed"),
    detector: key(input.detector_device_seed_hex, "detector Device seed"),
  };
  const snapshotContent = array([
    uint(VERSION), bytes(keys.root.publicKey), uint(input.authority_revision), array([]),
  ]);
  const snapshot = signed(DOMAINS.authoritySnapshot, snapshotContent, keys.root, true);
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
  return { keys, roster: signed(DOMAINS.deviceList, rosterContent, keys.root, true) };
}
function verifyEventDependency(event) {
  const expected = event.expected;
  const authorRoot = key(event.input.author_root_secret_key_hex, "event author Account seed");
  const authorDevice = key(event.input.author_device_secret_key_hex, "event author Device seed");
  assert(authorRoot.publicKey.toString("hex") === expected.author_account_id_hex,
    "event author Account key diverged");
  assert(authorDevice.publicKey.toString("hex") === expected.author_device_id_hex,
    "event author Device key diverged");
  const conversation = fromHex(event.input.conversation_id_hex, 32, "conversation ID");
  const recipientAccount = fromHex(expected.recipient_account_id_hex, 32, "recipient Account ID");
  const members = [authorRoot.publicKey, recipientAccount]
    .sort((left, right) => Buffer.compare(left, right));
  const membershipContent = array([
    uint(VERSION), bytes(conversation), uint(1), bytes(authorRoot.publicKey),
    array(members.map(bytes)),
  ]);
  const membership = signed(DOMAINS.membership, membershipContent, authorRoot, true);
  assert(membership.content.toString("hex") === expected.membership.content_cbor_hex,
    "membership content diverged");
  assert(membership.cbor.toString("hex") === expected.membership.signed_cbor_hex,
    "membership envelope diverged");

  const eventContent = fromHex(expected.text_event.content_cbor_hex, undefined, "event content");
  const eventSignature = fromHex(expected.text_event.signature_hex, 64, "event signature");
  const eventPreimage = Buffer.concat([DOMAINS.eventSignature, eventContent]);
  assert(eventPreimage.toString("hex") === expected.text_event.signing_preimage_hex,
    "event signing preimage diverged");
  assert(verifyEd25519(null, eventPreimage, publicFromBytes(authorDevice.publicKey), eventSignature),
    "event signature failed");
  const eventEnvelope = array([uint(VERSION), eventContent, bytes(eventSignature)]);
  assert(eventEnvelope.toString("hex") === expected.text_event.signed_cbor_hex,
    "event envelope diverged");
  const eventId = domainHash(DOMAINS.eventId, eventEnvelope);
  assert(eventId.toString("hex") === expected.text_event.event_id_hex, "Event ID diverged");
  return {
    conversation,
    eventId,
    membership: membership.cbor,
    authorizedEvent: fromHex(expected.authorized_text_event_cbor_hex, undefined,
      "authorized event"),
    recipientAccount,
  };
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function objectJson(cbor, objectId) {
  return { cbor_hex: cbor.toString("hex"), object_id_hex: objectId.toString("hex") };
}

function generate(input, endpoint, event) {
  const { keys, roster } = buildRoster(endpoint);
  const dependency = verifyEventDependency(event);
  assert(keys.root.publicKey.equals(dependency.recipientAccount),
    "rewrap Account is not a member of the event conversation");
  const session = fromHex(input.session_binding_hex, 32, "session binding");
  const encapsulatedKey = fromHex(input.encapsulated_key_hex, 32, "HPKE encapsulated key");
  const ciphertext = fromHex(input.ciphertext_hex, undefined, "HPKE ciphertext");
  assert(ciphertext.length > 16, "HPKE ciphertext is too short");

  const inventoryInput = array([
    uint(VERSION), bytes(dependency.conversation), array([bytes(dependency.eventId)]),
  ]);
  const inventoryDigest = domainHash(DOMAINS.inventory, inventoryInput);
  const sasInput = array([
    uint(VERSION), bytes(roster.cbor), bytes(keys.source.publicKey), bytes(keys.recipient.publicKey),
  ]);
  const sas = domainHash(DOMAINS.sas, sasInput);
  const manifest = array([
    uint(VERSION), bytes(dependency.conversation), bytes(roster.cbor),
    bytes(dependency.membership), bytes(keys.source.publicKey), bytes(keys.recipient.publicKey),
    uint(1), bytes(inventoryDigest), uint(input.range_start), uint(input.range_start + 1),
  ]);
  const manifestId = domainHash(DOMAINS.manifestId, manifest);
  const aadInput = array([
    uint(VERSION), bytes(manifestId), uint(input.range_start), bytes(dependency.eventId),
  ]);
  const aad = Buffer.concat([DOMAINS.entryAad, aadInput]);
  const entryContent = array([
    uint(VERSION), bytes(manifestId), bytes(keys.source.publicKey), uint(input.range_start),
    bytes(dependency.authorizedEvent), bytes(encapsulatedKey), bytes(ciphertext),
  ]);
  const entry = signed(DOMAINS.entrySignature, entryContent, keys.source, true);
  const bundle = array([uint(VERSION), bytes(manifest), array([bytes(entry.cbor)])]);
  const bundleId = domainHash(DOMAINS.bundleId, bundle);
  const requestContent = array([
    uint(VERSION), bytes(dependency.conversation), bytes(keys.source.publicKey),
    bytes(keys.recipient.publicKey), bytes(session), uint(input.range_start),
    uint(input.max_event_count), bytes(sas),
  ]);
  const request = signed(DOMAINS.request, requestContent, keys.recipient, true);
  const transferContent = array([uint(VERSION), bytes(request.cbor), bytes(bundle)]);
  const transfer = signed(DOMAINS.transfer, transferContent, keys.source, true);

  return {
    account_id_hex: keys.root.publicKey.toString("hex"),
    source_device_id_hex: keys.source.publicKey.toString("hex"),
    recipient_device_id_hex: keys.recipient.publicKey.toString("hex"),
    recipient_encryption_public_key_hex: endpoint.input.recipient_encryption_public_key_hex,
    conversation_id_hex: dependency.conversation.toString("hex"),
    event_id_hex: dependency.eventId.toString("hex"),
    roster_cbor_hex: roster.cbor.toString("hex"),
    membership_cbor_hex: dependency.membership.toString("hex"),
    authorized_event_cbor_hex: dependency.authorizedEvent.toString("hex"),
    sas: {
      input_cbor_hex: sasInput.toString("hex"),
      preimage_hex: Buffer.concat([DOMAINS.sas, sasInput]).toString("hex"),
      digest_hex: sas.toString("hex"),
    },
    inventory: {
      input_cbor_hex: inventoryInput.toString("hex"),
      preimage_hex: Buffer.concat([DOMAINS.inventory, inventoryInput]).toString("hex"),
      digest_hex: inventoryDigest.toString("hex"),
    },
    request: signedJson(request),
    manifest: {
      ...objectJson(manifest, manifestId),
      id_preimage_hex: Buffer.concat([DOMAINS.manifestId, manifest]).toString("hex"),
    },
    hpke: {
      info_hex: DOMAINS.hpkeInfo.toString("hex"),
      aad_input_cbor_hex: aadInput.toString("hex"),
      aad_hex: aad.toString("hex"),
      plaintext_utf8: input.plaintext_utf8,
      encapsulated_key_hex: encapsulatedKey.toString("hex"),
      ciphertext_hex: ciphertext.toString("hex"),
      implementation_note: "sealed-and-opened-by-rust-kilogram-crypto",
    },
    entry: signedJson(entry),
    bundle: {
      ...objectJson(bundle, bundleId),
      id_preimage_hex: Buffer.concat([DOMAINS.bundleId, bundle]).toString("hex"),
    },
    transfer: signedJson(transfer),
  };
}

function parse(argv) {
  const result = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--endpoint-vectors", "--event-vectors", "--output"]
      .includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["endpoint-vectors"], "--endpoint-vectors is required");
  assert(result["event-vectors"], "--event-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  session_binding_hex: "f0e0d0c0b0a09080706050403020100000102030405060708090a0b0c0d0e0f0",
  range_start: 0,
  max_event_count: 1,
  plaintext_utf8: "canonical history body",
  encapsulated_key_hex: "18546dcf1d69e2a22cdcbdff2edad7b970fa494f4c517d9ddd24e552e98ecf3f",
  ciphertext_hex: "b5f10e2b0519775f8065646a5d463dea8d9a0fb859b2ff6c1c81b98b904a5439bb95bd1e11eb",
};
const options = parse(process.argv.slice(2));
const endpoint = JSON.parse(readFileSync(options["endpoint-vectors"], "utf8"));
const event = JSON.parse(readFileSync(options["event-vectors"], "utf8"));
if (options.emit) {
  const generated = `${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependencies: ["CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json", "CANONICAL-EVENT-VECTORS.json"],
    input: defaultInput,
    expected: generate(defaultInput, endpoint, event),
  }, null, 2)}\n`;
  if (options.output) {
    writeFileSync(options.output, generated, "utf8");
    process.stdout.write(`canonical_history_rewrap_vectors_written=${options.output}\n`);
  } else process.stdout.write(generated);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.25" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "history-rewrap schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "history-rewrap vector header changed");
equalJson(generate(vectors.input, endpoint, event), vectors.expected,
  "independent history-rewrap vector");
if (options.selfTest) {
  for (const [label, mutate] of [
    ["changed_session", (value) => { value.input.session_binding_hex =
      `00${value.input.session_binding_hex.slice(2)}`; }],
    ["changed_ciphertext", (value) => { value.input.ciphertext_hex =
      `00${value.input.ciphertext_hex.slice(2)}`; }],
    ["changed_transfer_signature", (value) => { value.expected.transfer.signature_hex =
      `00${value.expected.transfer.signature_hex.slice(2)}`; }],
  ]) {
    const changed = JSON.parse(JSON.stringify(vectors)); mutate(changed);
    let rejected = false;
    try { equalJson(generate(changed.input, endpoint, event), changed.expected, label); }
    catch { rejected = true; }
    assert(rejected, `${label} was accepted`);
  }
  process.stdout.write("canonical_history_rewrap_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_history_rewrap_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519+javascript-blake3\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("hpke_fixture_opened_by_rust=true\n");
  process.stdout.write("independent_hpke_seal_open=pending-before-activation\n");
  process.stdout.write("active_wire_changed=false\n");
}
