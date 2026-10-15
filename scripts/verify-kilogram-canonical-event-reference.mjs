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
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const EVENT_SIGNATURE_DOMAIN = Buffer.from("kilogram:event-signature:cbor-v1\0", "utf8");
const EVENT_ID_DOMAIN = Buffer.from("kilogram:event-id:cbor-v1\0", "utf8");
const MEMBERSHIP_DOMAIN = Buffer.from("kilogram:conversation-membership-signature:cbor-v1\0", "utf8");

function fail(message) { throw new Error(message); }
function assert(condition, message) { if (!condition) fail(message); }
function head(major, value) {
  assert(Number.isSafeInteger(value) && value >= 0, "invalid CBOR argument");
  const prefix = major << 5;
  if (value <= 23) return Buffer.from([prefix | value]);
  if (value <= 0xff) return Buffer.from([prefix | 24, value]);
  if (value <= 0xffff) { const result = Buffer.alloc(3); result[0] = prefix | 25; result.writeUInt16BE(value, 1); return result; }
  if (value <= 0xffffffff) { const result = Buffer.alloc(5); result[0] = prefix | 26; result.writeUInt32BE(value, 1); return result; }
  const result = Buffer.alloc(9); result[0] = prefix | 27; result.writeBigUInt64BE(BigInt(value), 1); return result;
}
const uint = (value) => head(0, value);
const bytes = (value) => Buffer.concat([head(2, value.length), value]);
const array = (values) => Buffer.concat([head(4, values.length), ...values]);
function fromHex(value, size, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} is not lowercase hexadecimal`);
  const result = Buffer.from(value, "hex");
  if (size !== undefined) assert(result.length === size, `${label} has wrong length`);
  return result;
}
function privateFromSeed(seed) {
  return createPrivateKey({ key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8" });
}
function publicBytes(privateKey) {
  const spki = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected SPKI prefix");
  return spki.subarray(SPKI_PREFIX.length);
}
function publicFromBytes(raw) {
  return createPublicKey({ key: Buffer.concat([SPKI_PREFIX, raw]), format: "der", type: "spki" });
}
function signed(domain, content, privateKey) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, privateKey);
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  return { content, preimage, signature, envelope };
}
function eventExpected(value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: EVENT_SIGNATURE_DOMAIN.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
    event_id_domain_hex: EVENT_ID_DOMAIN.toString("hex"),
    event_id_hex: blake3Hash(Buffer.concat([EVENT_ID_DOMAIN, value.envelope])).toString("hex"),
  };
}
function signedExpected(domain, value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: domain.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
  };
}
function generate(ratchet, account, recipientAuthority, input) {
  const rootPrivate = privateFromSeed(fromHex(input.author_root_secret_key_hex, 32, "author Root seed"));
  const devicePrivate = privateFromSeed(fromHex(input.author_device_secret_key_hex, 32, "author Device seed"));
  const authorAccount = publicBytes(rootPrivate);
  const authorDevice = publicBytes(devicePrivate);
  assert(authorAccount.toString("hex") === ratchet.expected.account_id_hex, "author Account fixture mismatch");
  assert(authorDevice.toString("hex") === ratchet.expected.device_id_hex, "author Device fixture mismatch");
  const recipientAccount = fromHex(account.expected.account_id_hex, 32, "recipient Account");
  assert(recipientAccount.toString("hex") === recipientAuthority.expected.account_id_hex,
    "recipient Account authority fixture mismatch");
  const recipientDeviceId = fromHex(recipientAuthority.expected.device_id_hex, 32, "recipient Device");
  const conversation = fromHex(input.conversation_id_hex, 32, "conversation ID");
  const members = [authorAccount, recipientAccount].sort(Buffer.compare);
  const membershipContent = array([
    uint(VERSION), bytes(conversation), uint(1), bytes(authorAccount), array(members.map(bytes)),
  ]);
  const membership = signed(MEMBERSHIP_DOMAIN, membershipContent, rootPrivate);

  const parents = input.parents_hex.map((value) => fromHex(value, 32, "parent"));
  const deviceList = fromHex(account.expected.account_device_list.signed_cbor_hex, undefined, "device list");
  const ratchetIdentity = fromHex(ratchet.expected.ratchet_identity.signed_cbor_hex, undefined, "ratchet identity");
  const recipient = array([
    uint(VERSION), bytes(recipientDeviceId), uint(input.recipient_message_kind),
    bytes(fromHex(input.recipient_ciphertext_hex, undefined, "ciphertext")),
  ]);
  const textPayload = array([
    uint(2), bytes(deviceList), bytes(ratchetIdentity), array([recipient]),
  ]);
  const textContent = array([
    uint(VERSION), bytes(conversation), bytes(authorDevice), uint(input.text_author_sequence),
    array(parents.map(bytes)), textPayload,
  ]);
  const textEvent = signed(EVENT_SIGNATURE_DOMAIN, textContent, devicePrivate);
  const textExpected = eventExpected(textEvent);
  const certificate = fromHex(ratchet.expected.certificate.signed_cbor_hex, undefined, "author certificate");
  const snapshot = fromHex(ratchet.expected.authority_snapshot.signed_cbor_hex, undefined, "authority snapshot");
  const authorizedText = array([
    uint(VERSION), bytes(textEvent.envelope), bytes(certificate), bytes(snapshot),
  ]);

  const textId = fromHex(textExpected.event_id_hex, 32, "text event ID");
  const acknowledgementPayload = array([uint(1), bytes(textId)]);
  const acknowledgementContent = array([
    uint(VERSION), bytes(conversation), bytes(authorDevice),
    uint(input.acknowledgement_author_sequence), array([bytes(textId)]), acknowledgementPayload,
  ]);
  const acknowledgementEvent = signed(EVENT_SIGNATURE_DOMAIN, acknowledgementContent, devicePrivate);
  const authorizedAcknowledgement = array([
    uint(VERSION), bytes(acknowledgementEvent.envelope), bytes(certificate), bytes(snapshot),
  ]);
  return {
    author_account_id_hex: authorAccount.toString("hex"),
    author_device_id_hex: authorDevice.toString("hex"),
    recipient_account_id_hex: recipientAccount.toString("hex"),
    recipient_device_id_hex: recipientDeviceId.toString("hex"),
    membership: signedExpected(MEMBERSHIP_DOMAIN, membership),
    text_event: textExpected,
    authorized_text_event_cbor_hex: authorizedText.toString("hex"),
    acknowledgement_event: eventExpected(acknowledgementEvent),
    authorized_acknowledgement_event_cbor_hex: authorizedAcknowledgement.toString("hex"),
  };
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function verifyEvent(name, expected, deviceId) {
  const content = fromHex(expected.content_cbor_hex, undefined, `${name} content`);
  const preimage = Buffer.concat([EVENT_SIGNATURE_DOMAIN, content]);
  assert(preimage.toString("hex") === expected.signing_preimage_hex, `${name} preimage mismatch`);
  assert(verifyEd25519(null, preimage, publicFromBytes(deviceId),
    fromHex(expected.signature_hex, 64, `${name} signature`)), `${name} signature failed`);
  const envelope = array([uint(VERSION), content, bytes(fromHex(expected.signature_hex, 64, `${name} signature`))]);
  assert(envelope.toString("hex") === expected.signed_cbor_hex, `${name} envelope mismatch`);
  assert(blake3Hash(Buffer.concat([EVENT_ID_DOMAIN, envelope])).toString("hex") === expected.event_id_hex,
    `${name} event ID mismatch`);
}
function parse(argv) {
  const value = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") value.emit = true;
    else if (argument === "--self-test") value.selfTest = true;
    else if (["--schema", "--vectors", "--ratchet-vectors", "--account-vectors", "--recipient-authority-vectors"].includes(argument)) {
      value[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(value["ratchet-vectors"], "--ratchet-vectors is required");
  assert(value["account-vectors"], "--account-vectors is required");
  assert(value["recipient-authority-vectors"], "--recipient-authority-vectors is required");
  if (!value.emit) { assert(value.schema, "--schema is required"); assert(value.vectors, "--vectors is required"); }
  return value;
}

const options = parse(process.argv.slice(2));
const ratchet = JSON.parse(readFileSync(options["ratchet-vectors"], "utf8"));
const account = JSON.parse(readFileSync(options["account-vectors"], "utf8"));
const recipientAuthority = JSON.parse(readFileSync(options["recipient-authority-vectors"], "utf8"));
const defaultInput = {
  author_root_secret_key_hex: ratchet.input.root_secret_key_hex,
  author_device_secret_key_hex: ratchet.input.device_secret_key_hex,
  conversation_id_hex: "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeef",
  text_author_sequence: 9,
  acknowledgement_author_sequence: 10,
  parents_hex: [
    "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f",
    "303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f",
  ],
  recipient_ciphertext_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  recipient_message_kind: 0,
};
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1, profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active", input: defaultInput,
    expected: generate(ratchet, account, recipientAuthority, defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.10" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "event schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "event vector header changed");
const generated = generate(ratchet, account, recipientAuthority, vectors.input);
equalJson(generated, vectors.expected, "independent event vector");
const authorDevice = fromHex(generated.author_device_id_hex, 32, "author Device");
verifyEvent("text event", generated.text_event, authorDevice);
verifyEvent("acknowledgement event", generated.acknowledgement_event, authorDevice);
const membership = generated.membership;
assert(verifyEd25519(null, fromHex(membership.signing_preimage_hex, undefined, "membership preimage"),
  publicFromBytes(fromHex(generated.author_account_id_hex, 32, "author Account")),
  fromHex(membership.signature_hex, 64, "membership signature")), "membership signature failed");
if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.input.recipient_ciphertext_hex = `00${changed.input.recipient_ciphertext_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(ratchet, account, recipientAuthority, changed.input), changed.expected, "changed ciphertext"); } catch { rejected = true; }
  assert(rejected, "changed ciphertext was accepted");
  process.stdout.write("canonical_event_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_event_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519-and-independent-blake3\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("active_wire_changed=false\n");
}
