import { createPrivateKey, createPublicKey, verify as verifyEd25519 } from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const domains = Object.fromEntries(Object.entries({
  descriptorId: "kilogram:runtime-ipc-descriptor-id:cbor-v1\0",
  descriptorSignature: "kilogram:runtime-ipc-descriptor-signature:cbor-v1\0",
  providerResponseId: "kilogram:runtime-ipc-provider-response-id:cbor-v1\0",
  providerResponseSignature: "kilogram:runtime-ipc-provider-response-signature:cbor-v1\0",
  storageOfferSignature: "kilogram:blind-mailbox-storage-offer:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-mailbox-mutation-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-mailbox-mutation-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-mailbox-mutation-response-id:cbor-v1\0",
  updateSignature: "kilogram:mailbox-capability-update:cbor-v1\0",
  updateId: "kilogram:mailbox-capability-update-id:cbor-v1\0",
  acknowledgementSignature: "kilogram:mailbox-capability-acknowledgement:cbor-v1\0",
}).map(([name, value]) => [name, Buffer.from(value)]));

function fail(message) { throw new Error(message); }
function assert(value, message) { if (!value) fail(message); }
function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} is not canonical lowercase hexadecimal`);
  const result = Buffer.from(value, "hex");
  if (length !== undefined) assert(result.length === length, `${label} length changed`);
  return result;
}
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function publicFromSeed(seedHex) {
  const seed = fromHex(seedHex, 32, "Ed25519 seed");
  const privateKey = createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8",
  });
  const der = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(der.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI");
  return der.subarray(SPKI_PREFIX.length);
}
function publicFromBytes(raw) {
  return createPublicKey({ key: Buffer.concat([SPKI_PREFIX, raw]), format: "der", type: "spki" });
}

function argument(buffer, offset, additional) {
  if (additional < 24) return { value: BigInt(additional), next: offset };
  if (additional === 24) {
    assert(offset < buffer.length && buffer[offset] >= 24, "non-minimal uint8 CBOR argument");
    return { value: BigInt(buffer[offset]), next: offset + 1 };
  }
  if (additional === 25) {
    assert(offset + 2 <= buffer.length, "truncated uint16 CBOR argument");
    const value = buffer.readUInt16BE(offset);
    assert(value > 0xff, "non-minimal uint16 CBOR argument");
    return { value: BigInt(value), next: offset + 2 };
  }
  if (additional === 26) {
    assert(offset + 4 <= buffer.length, "truncated uint32 CBOR argument");
    const value = buffer.readUInt32BE(offset);
    assert(value > 0xffff, "non-minimal uint32 CBOR argument");
    return { value: BigInt(value), next: offset + 4 };
  }
  if (additional === 27) {
    assert(offset + 8 <= buffer.length, "truncated uint64 CBOR argument");
    const value = buffer.readBigUInt64BE(offset);
    assert(value > 0xffffffffn, "non-minimal uint64 CBOR argument");
    return { value, next: offset + 8 };
  }
  fail("indefinite or unsupported CBOR argument");
}

function decodeAt(buffer, start) {
  assert(start < buffer.length, "truncated CBOR item");
  const initial = buffer[start];
  const major = initial >> 5;
  const parsed = argument(buffer, start + 1, initial & 31);
  const count = Number(parsed.value);
  assert(Number.isSafeInteger(count), "CBOR argument exceeds safe reference range");
  let end = parsed.next;
  if (major === 0) return { kind: "uint", uint: parsed.value, start, end, raw: buffer.subarray(start, end) };
  if (major === 2) {
    assert(end + count <= buffer.length, "truncated CBOR byte string");
    const data = buffer.subarray(end, end + count); end += count;
    return { kind: "bytes", data, start, end, raw: buffer.subarray(start, end) };
  }
  if (major === 4) {
    const children = [];
    for (let index = 0; index < count; index += 1) {
      const child = decodeAt(buffer, end); children.push(child); end = child.end;
    }
    return { kind: "array", children, start, end, raw: buffer.subarray(start, end) };
  }
  fail(`unsupported CBOR major type ${major}`);
}
function decode(buffer) {
  const result = decodeAt(buffer, 0);
  assert(result.end === buffer.length, "trailing CBOR data");
  return result;
}
function arrayNode(node, length, label) {
  assert(node.kind === "array" && (length === undefined || node.children.length === length),
    `${label} shape changed`);
  return node.children;
}
function uintNode(node, expected, label) {
  assert(node.kind === "uint", `${label} is not unsigned`);
  if (expected !== undefined) assert(node.uint === BigInt(expected), `${label} changed`);
  return node.uint;
}
function bytesNode(node, length, label) {
  assert(node.kind === "bytes" && (length === undefined || node.data.length === length),
    `${label} byte string changed`);
  return node.data;
}
function equalBytes(left, right, label) { assert(left.equals(right), `${label} changed`); }
function verifyEnvelope(encoded, domain, publicKey, label) {
  const envelope = arrayNode(decode(encoded), 3, label);
  uintNode(envelope[0], 1, `${label} version`);
  const signature = bytesNode(envelope[2], 64, `${label} signature`);
  const contentBytes = envelope[1].kind === "bytes" ? envelope[1].data : envelope[1].raw;
  assert(verifyEd25519(null, Buffer.concat([domain, contentBytes]),
    publicFromBytes(publicKey), signature), `${label} signature failed`);
  const content = envelope[1].kind === "bytes" ? decode(contentBytes) : envelope[1];
  return { envelope, content, signature };
}
function option(node, label) {
  assert(node.kind === "array" && (node.children.length === 1 || node.children.length === 2),
    `${label} option shape changed`);
  const tag = Number(uintNode(node.children[0], undefined, `${label} tag`));
  assert((tag === 0 && node.children.length === 1) || (tag === 1 && node.children.length === 2),
    `${label} option tag changed`);
  return tag === 0 ? undefined : node.children[1];
}
function plainOption(node, label) {
  assert(node.kind === "array" && (node.children.length === 0 || node.children.length === 1),
    `${label} option shape changed`);
  return node.children[0];
}

function verifyUpdate(encoded, ownerDevice, label) {
  const signed = verifyEnvelope(encoded, domains.updateSignature, ownerDevice, label);
  const content = arrayNode(signed.content, 10, `${label} content`);
  uintNode(content[0], 1, `${label} content version`);
  equalBytes(bytesNode(content[2], 32, `${label} owner Device`), ownerDevice,
    `${label} owner Device`);
  const actionBytes = bytesNode(content[9], undefined, `${label} action`);
  const action = arrayNode(decode(actionBytes), undefined, `${label} action`);
  return { encoded, content, action, id: domainHash(domains.updateId, encoded) };
}
function verifyHead(node, ownerDevice, recipientDevice, label) {
  const head = arrayNode(node, 3, label);
  uintNode(head[0], 1, `${label} version`);
  const update = verifyUpdate(bytesNode(head[1], undefined, `${label} update`), ownerDevice,
    `${label} update`);
  const acknowledgementNode = option(head[2], `${label} acknowledgement`);
  assert(acknowledgementNode !== undefined, `${label} requires an acknowledgement`);
  const acknowledgementBytes = bytesNode(acknowledgementNode, undefined, `${label} acknowledgement`);
  const acknowledgement = verifyEnvelope(acknowledgementBytes,
    domains.acknowledgementSignature, recipientDevice, `${label} acknowledgement`);
  const acknowledgementContent = arrayNode(acknowledgement.content, 10,
    `${label} acknowledgement content`);
  equalBytes(bytesNode(acknowledgementContent[2], 32, `${label} acknowledged update ID`),
    update.id, `${label} acknowledged update ID`);
  uintNode(acknowledgementContent[9], 0, `${label} acknowledged revoked flag`);
  return update;
}
function verifyProviderSelection(encoded, ownerDevice) {
  const selection = verifyEnvelope(encoded, domains.providerResponseSignature, ownerDevice,
    "provider selection");
  const content = arrayNode(selection.content, 6, "provider selection content");
  const result = arrayNode(content[5], 5, "provider selection result");
  uintNode(result[0], 1, "provider selection result tag");
  uintNode(result[2], 3, "provider selection requested count");
  const providers = arrayNode(result[4], 3, "provider selection providers");
  const keys = [];
  const transports = new Set();
  let minimumExpiry;
  for (const [index, providerNode] of providers.entries()) {
    const provider = arrayNode(providerNode, 3, `provider ${index}`);
    const offerBytes = bytesNode(provider[0], undefined, `provider ${index} offer`);
    const offerRoot = decode(offerBytes);
    const offerEnvelope = arrayNode(offerRoot, 3, `provider ${index} offer envelope`);
    const offerContent = arrayNode(offerEnvelope[1], 9, `provider ${index} offer content`);
    const storeKey = bytesNode(offerContent[2], 32, `provider ${index} store key`);
    verifyEnvelope(offerBytes, domains.storageOfferSignature, storeKey, `provider ${index} offer`);
    keys.push(storeKey);
    const transport = bytesNode(provider[1], 32, `provider ${index} transport`).toString("hex");
    assert(!transports.has(transport), "duplicate provider transport identity"); transports.add(transport);
    const expiry = uintNode(offerContent[7], undefined, `provider ${index} expiry`);
    minimumExpiry = minimumExpiry === undefined || expiry < minimumExpiry ? expiry : minimumExpiry;
  }
  for (let index = 1; index < keys.length; index += 1)
    assert(Buffer.compare(keys[index - 1], keys[index]) < 0, "provider store keys are not ordered");
  return { keys, minimumExpiry, id: domainHash(domains.providerResponseId, encoded), content };
}

function parseOptions(argv) {
  const result = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--provisioning-vectors"].includes(value)) {
      result[value.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${value}`);
  }
  for (const name of ["schema", "vectors", "provisioning-vectors"])
    assert(result[name], `--${name} is required`);
  return result;
}

function verifyCase(name, expected, context) {
  const requestBytes = fromHex(expected.request.cbor_hex, undefined, `${name} request`);
  const request = arrayNode(decode(requestBytes), 7, `${name} request`);
  uintNode(request[0], 1, `${name} request version`);
  equalBytes(bytesNode(request[1], 32, `${name} descriptor ID`), context.descriptorId,
    `${name} descriptor ID`);
  const requestId = bytesNode(request[2], 32, `${name} request ID`);
  const requestDigest = domainHash(domains.requestDigest, requestBytes);
  equalBytes(requestDigest, fromHex(expected.request.object_id_hex, 32, `${name} request digest`),
    `${name} request digest`);
  const command = request[6].children;
  const expectedTag = { create: 0, rotate: 1, revoke: 2 }[name];
  uintNode(command[0], expectedTag, `${name} command tag`);
  const target = command[1];
  const targetFields = arrayNode(target, 6, `${name} target`);
  equalBytes(bytesNode(targetFields[2], 32, `${name} recipient Account`), context.recipientAccount,
    `${name} recipient Account`);
  equalBytes(bytesNode(targetFields[3], 32, `${name} recipient Device`), context.recipientDevice,
    `${name} recipient Device`);

  let previous;
  let selection;
  let validity;
  if (name === "create") {
    selection = verifyProviderSelection(bytesNode(command[2], undefined, "create provider selection"),
      context.ownerDevice);
    validity = uintNode(command[3], 3500, "create offer validity");
  } else if (name === "rotate") {
    previous = verifyHead(command[2], context.ownerDevice, context.recipientDevice,
      "rotate predecessor");
    selection = verifyProviderSelection(bytesNode(command[3], undefined, "rotate provider selection"),
      context.ownerDevice);
    validity = uintNode(command[4], 3500, "rotate offer validity");
  } else {
    previous = verifyHead(command[2], context.ownerDevice, context.recipientDevice,
      "revoke predecessor");
  }
  if (selection) {
    equalBytes(selection.id, context.selection.id, `${name} selection ID`);
    for (let index = 0; index < selection.keys.length; index += 1)
      equalBytes(selection.keys[index], context.selection.keys[index], `${name} provider ${index}`);
  }

  const responseBytes = fromHex(expected.response.signed_cbor_hex, undefined, `${name} response`);
  const response = verifyEnvelope(responseBytes, domains.responseSignature, context.ownerDevice,
    `${name} response`);
  const responseContent = arrayNode(response.content, 6, `${name} response content`);
  equalBytes(bytesNode(responseContent[1], 32, `${name} response descriptor ID`),
    context.descriptorId, `${name} response descriptor ID`);
  equalBytes(bytesNode(responseContent[2], 32, `${name} response request ID`), requestId,
    `${name} response request ID`);
  equalBytes(bytesNode(responseContent[3], 32, `${name} response request digest`), requestDigest,
    `${name} response request digest`);
  const result = arrayNode(responseContent[5], 2, `${name} response result`);
  uintNode(result[0], 0, `${name} response result tag`);
  const transition = arrayNode(result[1], 7, `${name} transition`);
  uintNode(transition[0], 1, `${name} transition version`);
  uintNode(transition[1], 0, `${name} transition outcome`);
  equalBytes(transition[2].raw, target.raw, `${name} transition target`);
  const update = verifyUpdate(bytesNode(transition[3], undefined, `${name} transition update`),
    context.ownerDevice, `${name} transition update`);
  equalBytes(bytesNode(update.content[3], 32, `${name} update recipient Account`),
    context.recipientAccount, `${name} update recipient Account`);
  equalBytes(bytesNode(update.content[4], 32, `${name} update recipient Device`),
    context.recipientDevice, `${name} update recipient Device`);
  equalBytes(bytesNode(update.content[5], 32, `${name} update scope`),
    bytesNode(targetFields[5], 32, `${name} target scope`), `${name} scope`);
  const generation = uintNode(update.content[6], undefined, `${name} generation`);
  const predecessorOption = plainOption(update.content[7], `${name} predecessor`);
  const createdAt = uintNode(update.content[8], undefined, `${name} created at`);
  if (name === "create") {
    assert(generation === 1n && predecessorOption === undefined, "create chain origin changed");
  } else {
    assert(generation === 2n && predecessorOption !== undefined, `${name} generation changed`);
    equalBytes(bytesNode(predecessorOption, 32, `${name} predecessor ID`), previous.id,
      `${name} predecessor ID`);
  }
  if (name !== "revoke") {
    assert(update.action.length === 6, `${name} activation action shape changed`);
    uintNode(update.action[1], 1, `${name} activation tag`);
    uintNode(update.action[2], 2, `${name} exact-volunteer mode`);
    const replicaOption = plainOption(update.action[5], `${name} replica set`);
    assert(replicaOption !== undefined, `${name} replica set missing`);
    const replicaSet = arrayNode(decode(bytesNode(replicaOption, undefined, `${name} replica set`)),
      2, `${name} replica set`);
    const replicaKeys = arrayNode(replicaSet[1], 3, `${name} replica keys`)
      .map((node, index) => bytesNode(node, 32, `${name} replica key ${index}`));
    for (let index = 0; index < replicaKeys.length; index += 1)
      equalBytes(replicaKeys[index], selection.keys[index], `${name} replica key ${index}`);
    const selectionOption = option(transition[4], `${name} provider selection ID`);
    equalBytes(bytesNode(selectionOption, 32, `${name} provider selection ID`), selection.id,
      `${name} provider selection ID`);
    const expiryOption = option(transition[5], `${name} offer expiry`);
    const expiry = uintNode(expiryOption, undefined, `${name} offer expiry`);
    assert(expiry === createdAt + validity && expiry <= selection.minimumExpiry,
      `${name} offer expiry is not provider-bounded`);
  } else {
    assert(update.action.length === 3, "revoke action shape changed");
    uintNode(update.action[1], 2, "revoke action tag");
    assert(option(transition[4], "revoke provider selection ID") === undefined,
      "revoke unexpectedly contains a provider selection");
    assert(option(transition[5], "revoke offer expiry") === undefined,
      "revoke unexpectedly contains an offer expiry");
    equalBytes(bytesNode(update.action[2], 32, "revoke binding ID"),
      bytesNode(previous.action[3], 32, "previous binding ID"), "revoke binding ID");
  }
  uintNode(transition[6], 0, `${name} delivery state`);
  equalBytes(domainHash(domains.responseId, responseBytes),
    fromHex(expected.response.object_id_hex, 32, `${name} response ID`), `${name} response ID`);
}

const options = parseOptions(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const provisioning = JSON.parse(readFileSync(options["provisioning-vectors"], "utf8"));
assert(schema.milestone === "M1.1.38" && schema.status === "candidate-not-active",
  "mailbox mutation schema milestone or activation state changed");
assert(vectors.schema_version === 1 && vectors.profile === "kilogram-deterministic-cbor-v1" &&
  vectors.activation_status === "not-active", "mailbox mutation vector header changed");
const ownerDevice = publicFromSeed(provisioning.input.owner_secret_key_hex);
const recipientDevice = publicFromSeed(provisioning.input.recipient_secret_key_hex);
const descriptorBytes = fromHex(vectors.expected.descriptor.signed_cbor_hex, undefined, "descriptor");
verifyEnvelope(descriptorBytes, domains.descriptorSignature, ownerDevice, "descriptor");
const descriptorId = domainHash(domains.descriptorId, descriptorBytes);
equalBytes(descriptorId, fromHex(vectors.expected.descriptor.object_id_hex, 32, "descriptor ID"),
  "descriptor ID");
const selectionBytes = fromHex(vectors.expected.provider_selection.signed_cbor_hex,
  undefined, "provider selection");
const selection = verifyProviderSelection(selectionBytes, ownerDevice);
equalBytes(selection.id,
  fromHex(vectors.expected.provider_selection.object_id_hex, 32, "provider selection ID"),
  "provider selection ID");
const context = {
  ownerDevice, recipientDevice, descriptorId, selection,
  recipientAccount: fromHex(provisioning.input.recipient_account_id_hex, 32,
    "recipient Account ID"),
};
for (const name of ["create", "rotate", "revoke"])
  verifyCase(name, vectors.expected[name], context);

if (options.selfTest) {
  const changed = Buffer.from(vectors.expected.create.request.cbor_hex, "hex"); changed[10] ^= 1;
  assert(!domainHash(domains.requestDigest, changed).equals(
    fromHex(vectors.expected.create.request.object_id_hex, 32, "create request ID")),
  "changed request was unexpectedly accepted");
  console.log("canonical_runtime_ipc_mailbox_mutation_reference_self_test=passed");
}
console.log("canonical_runtime_ipc_mailbox_mutation_reference=verified");
console.log("independent_blake3=verified");
