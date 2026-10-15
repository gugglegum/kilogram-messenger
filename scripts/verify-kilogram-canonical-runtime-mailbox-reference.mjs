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
const DOMAINS = {
  localSignature: Buffer.from("kilogram:runtime-local-mailbox-binding:cbor-v1\0"),
  peerSignature: Buffer.from("kilogram:runtime-peer-mailbox-binding:cbor-v1\0"),
  dispatchSignature: Buffer.from("kilogram:runtime-mailbox-dispatch:cbor-v1\0"),
  itemId: Buffer.from("kilogram:runtime-mailbox-item-id:cbor-v1\0"),
  eventItemId: Buffer.from("kilogram:runtime-mailbox-event-item-id:cbor-v1\0"),
  replicationBinding: Buffer.from(
    "kilogram:runtime-mailbox-dispatch-replication-binding:cbor-v1\0",
  ),
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
function privateKey(seed) {
  return createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8",
  });
}
function publicBytes(key) {
  const spki = createPublicKey(key).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX),
    "unexpected Ed25519 SPKI prefix");
  return spki.subarray(SPKI_PREFIX.length);
}
function sign(domain, content, key) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, key);
  assert(verifyEd25519(null, preimage, createPublicKey(key), signature),
    "generated runtime signature failed");
  return {
    content_cbor_hex: content.toString("hex"),
    signature_domain_hex: domain.toString("hex"),
    signing_preimage_hex: preimage.toString("hex"),
    signature_hex: signature.toString("hex"),
    signed_cbor_hex: array([uint(VERSION), bytes(content), bytes(signature)]).toString("hex"),
  };
}
function domainHash(domain, values) {
  return blake3Hash(Buffer.concat([domain, ...values]));
}
function verifySignature(publicKeyBytes, preimage, signature, label) {
  const publicKey = createPublicKey({
    key: Buffer.concat([SPKI_PREFIX, publicKeyBytes]), format: "der", type: "spki",
  });
  assert(verifyEd25519(null, preimage, publicKey, signature), `${label} signature failed`);
}
function verifyDependencies(provisioning, event) {
  const ownerDevice = fromHex(provisioning.expected.owner_device_id_hex, 32, "owner device");
  const local = provisioning.expected.signed_local_binding;
  verifySignature(ownerDevice,
    fromHex(local.signing_preimage_hex, undefined, "local binding preimage"),
    fromHex(local.signature_hex, 64, "local binding signature"), "local binding");
  const offer = provisioning.expected.signed_exact_peer_offer;
  verifySignature(ownerDevice,
    fromHex(offer.signing_preimage_hex, undefined, "peer offer preimage"),
    fromHex(offer.signature_hex, 64, "peer offer signature"), "peer offer");
  verifySignature(fromHex(event.expected.author_device_id_hex, 32, "event author device"),
    fromHex(event.expected.text_event.signing_preimage_hex, undefined, "event preimage"),
    fromHex(event.expected.text_event.signature_hex, 64, "event signature"), "event");
}

function generate(input, provisioning, event, mailbox) {
  verifyDependencies(provisioning, event);
  const ownerKey = privateKey(fromHex(provisioning.input.owner_secret_key_hex, 32, "owner seed"));
  const recipientKey = privateKey(
    fromHex(provisioning.input.recipient_secret_key_hex, 32, "recipient seed"),
  );
  const authorKey = privateKey(fromHex(event.input.author_device_secret_key_hex, 32, "author seed"));
  const ownerDevice = publicBytes(ownerKey);
  const recipientDevice = publicBytes(recipientKey);
  const authorDevice = publicBytes(authorKey);
  assert(ownerDevice.toString("hex") === provisioning.expected.owner_device_id_hex,
    "owner Device dependency changed");
  assert(recipientDevice.toString("hex") === provisioning.expected.recipient_device_id_hex,
    "recipient Device dependency changed");
  assert(authorDevice.toString("hex") === event.expected.author_device_id_hex,
    "event author Device dependency changed");

  const ownerAccount = fromHex(provisioning.input.owner_account_id_hex, 32, "owner account");
  const recipientAccount = fromHex(
    provisioning.input.recipient_account_id_hex, 32, "recipient account",
  );
  const scope = fromHex(provisioning.input.scope_hex, 32, "scope");
  const bindingId = fromHex(provisioning.expected.exact_binding_id_hex, 32, "binding ID");
  const localContact = fromHex(input.local_contact_id_hex, 32, "local contact ID");
  const peerContact = fromHex(input.peer_contact_id_hex, 32, "peer contact ID");
  const nestedLocal = fromHex(
    provisioning.expected.signed_local_binding.signed_cbor_hex, undefined, "nested local binding",
  );
  const nestedOffer = fromHex(
    provisioning.expected.signed_exact_peer_offer.signed_cbor_hex, undefined, "nested peer offer",
  );

  const localContent = array([
    uint(VERSION), bytes(ownerAccount), bytes(ownerDevice), bytes(localContact),
    bytes(recipientAccount), bytes(recipientDevice), bytes(scope), bytes(bindingId),
    uint(input.binding_created_at), bytes(nestedLocal),
  ]);
  const signedLocal = sign(DOMAINS.localSignature, localContent, ownerKey);
  const peerContent = array([
    uint(VERSION), bytes(recipientAccount), bytes(recipientDevice), bytes(peerContact),
    bytes(ownerAccount), bytes(ownerDevice), bytes(scope), bytes(bindingId),
    uint(input.offer_imported_at), bytes(nestedOffer),
  ]);
  const signedPeer = sign(DOMAINS.peerSignature, peerContent, recipientKey);

  const sourceAccount = fromHex(event.expected.author_account_id_hex, 32, "source account");
  const targetAccount = fromHex(event.expected.recipient_account_id_hex, 32, "target account");
  const targetDevice = fromHex(event.expected.recipient_device_id_hex, 32, "target device");
  const conversation = fromHex(event.input.conversation_id_hex, 32, "conversation");
  const authorizedEvent = fromHex(
    event.expected.authorized_text_event_cbor_hex, undefined, "authorized event",
  );
  const eventId = fromHex(event.expected.text_event.event_id_hex, 32, "event ID");
  const payload = array([
    uint(VERSION), bytes(bindingId), bytes(sourceAccount), bytes(authorDevice),
    bytes(targetAccount), bytes(targetDevice), bytes(conversation), bytes(authorizedEvent),
  ]);

  const queueId = fromHex(input.queue_id_hex, 32, "queue ID");
  const dispatchContact = fromHex(input.dispatch_contact_id_hex, 32, "dispatch contact ID");
  const mailboxId = fromHex(mailbox.expected.mailbox_id_hex, 32, "mailbox ID");
  const itemId = domainHash(DOMAINS.itemId, [queueId, eventId, bindingId]);
  const eventItemId = domainHash(DOMAINS.eventItemId, [eventId, bindingId]);
  const dispatchContent = array([
    uint(VERSION), bytes(sourceAccount), bytes(authorDevice), bytes(queueId),
    bytes(dispatchContact), bytes(targetAccount), bytes(targetDevice), bytes(conversation),
    bytes(bindingId), bytes(mailboxId), bytes(itemId), bytes(eventId),
    uint(input.dispatch_created_at), uint(input.dispatch_expires_at),
  ]);
  const signedDispatch = sign(DOMAINS.dispatchSignature, dispatchContent, authorKey);
  const signedDispatchBytes = fromHex(
    signedDispatch.signed_cbor_hex, undefined, "signed dispatch",
  );
  const replicationBinding = domainHash(DOMAINS.replicationBinding, [signedDispatchBytes]);

  return {
    domains: Object.fromEntries(
      Object.entries(DOMAINS).map(([name, value]) => [name, value.toString("hex")]),
    ),
    signed_local_binding: signedLocal,
    signed_peer_binding: signedPeer,
    payload: {
      cbor_hex: payload.toString("hex"),
      authorized_event_cbor_hex: authorizedEvent.toString("hex"),
    },
    signed_dispatch: {
      ...signedDispatch,
      item_id_hex: itemId.toString("hex"),
      event_item_id_hex: eventItemId.toString("hex"),
      replication_binding_hex: replicationBinding.toString("hex"),
    },
  };
}

function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function parse(argv) {
  const result = { emit: false, selfTest: false };
  const valued = new Set([
    "--schema", "--vectors", "--provisioning-vectors", "--event-vectors", "--mailbox-vectors",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") result.emit = true;
    else if (argument === "--self-test") result.selfTest = true;
    else if (valued.has(argument)) {
      result[argument.slice(2).replaceAll("-", "_")] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const name of ["provisioning_vectors", "event_vectors", "mailbox_vectors"]) {
    assert(result[name], `--${name.replaceAll("_", "-")} is required`);
  }
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const defaultInput = {
  local_contact_id_hex: "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
  peer_contact_id_hex: "505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f",
  queue_id_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  dispatch_contact_id_hex: "707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f",
  binding_created_at: 1800001100,
  offer_imported_at: 1800001200,
  dispatch_created_at: 1800001300,
  dispatch_expires_at: 1800004900,
};

const options = parse(process.argv.slice(2));
const provisioning = JSON.parse(readFileSync(options.provisioning_vectors, "utf8"));
const event = JSON.parse(readFileSync(options.event_vectors, "utf8"));
const mailbox = JSON.parse(readFileSync(options.mailbox_vectors, "utf8"));
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    dependencies: [
      "CANONICAL-MAILBOX-PROVISIONING-VECTORS.json",
      "CANONICAL-EVENT-VECTORS.json",
      "CANONICAL-MAILBOX-VECTORS.json",
    ],
    input: defaultInput,
    expected: generate(defaultInput, provisioning, event, mailbox),
  }, null, 2)}\n`);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.19" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "schema activation boundary changed");
assert(schema.cddl.path === "CANONICAL-RUNTIME-MAILBOX.cddl" &&
  schema.cddl.root_rule === "kilogram-runtime-mailbox-v1", "schema CDDL binding changed");
assert(schema.activation.current_runtime_encoding === "postcard-v1" &&
  schema.activation.current_runtime_changed === false, "active runtime mailbox changed");
equalJson(generate(vectors.input, provisioning, event, mailbox), vectors.expected,
  "canonical runtime mailbox vectors");

if (options.selfTest) {
  const changedQueue = structuredClone(vectors.input);
  changedQueue.queue_id_hex = `ff${changedQueue.queue_id_hex.slice(2)}`;
  assert(generate(changedQueue, provisioning, event, mailbox).signed_dispatch.item_id_hex !==
    vectors.expected.signed_dispatch.item_id_hex, "changed queue preserved item ID");
  const changedContact = structuredClone(vectors.input);
  changedContact.local_contact_id_hex = `ff${changedContact.local_contact_id_hex.slice(2)}`;
  assert(generate(changedContact, provisioning, event, mailbox).signed_local_binding.signature_hex !==
    vectors.expected.signed_local_binding.signature_hex, "changed contact preserved signature");
  process.stdout.write("changed_queue=rejected\n");
  process.stdout.write("changed_contact=rejected\n");
  process.stdout.write("canonical_runtime_mailbox_reference_self_test=passed\n");
}
process.stdout.write("independent_ed25519=verified\n");
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("nested_provisioning_and_event_signatures=verified\n");
process.stdout.write("canonical_runtime_mailbox_reference=verified\n");
