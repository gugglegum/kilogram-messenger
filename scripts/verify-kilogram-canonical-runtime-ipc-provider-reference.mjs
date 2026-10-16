import {
  createPrivateKey, createPublicKey, sign as signEd25519, verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  offerSignature: "kilogram:blind-mailbox-storage-offer:cbor-v1\0",
  admissionWork: "kilogram:blind-mailbox-storage-offer-admission-work:cbor-v1\0",
  offerId: "kilogram:mailbox-provider-offer-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-provider-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-provider-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-provider-response-id:cbor-v1\0",
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
  const seed = fromHex(seedHex, 32, `${label} seed`);
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
  return { content, preimage, signature, cbor: array([uint(VERSION), content, bytes(signature)]) };
}
function requestJson(value) {
  const digest = domainHash(DOMAINS.requestDigest, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.requestDigest, value]).toString("hex"),
    object_id_hex: digest.toString("hex"),
  };
}
function responseJson(value) {
  const id = domainHash(DOMAINS.responseId, value.cbor);
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: DOMAINS.responseSignature.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.responseId, value.cbor]).toString("hex"),
    object_id_hex: id.toString("hex"),
  };
}
function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value !== null && typeof value === "object")
    return Object.fromEntries(Object.keys(value).sort().map((name) => [name, stable(value[name])]));
  return value;
}
function equalJson(actual, expected, label) {
  if (JSON.stringify(stable(actual)) !== JSON.stringify(stable(expected)))
    fail(`${label} diverged`);
}
function leadingZeroBits(value) {
  let count = 0;
  for (const byte of value) {
    if (byte === 0) { count += 8; continue; }
    for (let mask = 0x80; (byte & mask) === 0; mask >>= 1) count += 1;
    break;
  }
  return count;
}

function generate(core, input) {
  const device = key(core.input.device_secret_key_hex, "Device");
  const store = key(input.store_secret_key_hex, "store");
  assert(device.publicKey.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  const descriptorId = fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID");
  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const selectionRequestId = fromHex(input.selection_request_id_hex, 32, "selection request ID");
  const bearer = fromHex(core.input.bearer_token_hex, 32, "bearer token");
  const endpoint = fromHex(input.provider_endpoint_hex, undefined, "provider endpoint");
  const nonce = fromHex(input.offer_nonce_hex, 32, "offer nonce");
  const offerContent = array([
    uint(VERSION), bytes(endpoint), bytes(store.publicKey), uint(1),
    uint(input.capacity_hint_bytes), uint(input.max_record_bytes),
    uint(input.offer_issued_at_unix_seconds), uint(input.offer_expires_at_unix_seconds), bytes(nonce),
  ]);
  const offer = signed(DOMAINS.offerSignature, offerContent, store);
  assert(offer.cbor.toString("hex") === input.offer_signed_cbor_hex,
    "signed provider offer dependency changed");
  assert(leadingZeroBits(domainHash(DOMAINS.admissionWork, offerContent)) >= 8,
    "provider offer no longer satisfies admission work");
  const offerId = domainHash(DOMAINS.offerId, offer.cbor);
  const command = array([uint(0), bytes(offer.cbor)]);
  const request = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds), command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);
  const provider = array([
    bytes(offer.cbor), bytes(fromHex(input.transport_identity_hex, 32, "transport identity")),
    uint(input.observed_at_unix_seconds),
  ]);
  const result = array([uint(0), uint(0), uint(1), provider]);
  const content = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest),
    bytes(device.publicKey), result,
  ]);
  const response = signed(DOMAINS.responseSignature, content, device);
  const selectionSalt = fromHex(input.selection_salt_hex, 32, "selection salt");
  const selectionCommand = array([uint(1), bytes(selectionSalt), uint(2)]);
  const selectionRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(selectionRequestId), bytes(bearer),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds),
    selectionCommand,
  ]);
  const selectionRequestDigest = domainHash(DOMAINS.requestDigest, selectionRequest);
  const selectedProvider = array([
    bytes(offer.cbor), bytes(fromHex(input.transport_identity_hex, 32, "transport identity")),
    uint(input.selection_observed_at_unix_seconds),
  ]);
  const selectionResult = array([
    uint(1), bytes(selectionSalt), uint(2), uint(1), array([selectedProvider]),
  ]);
  const selectionContent = array([
    uint(VERSION), bytes(descriptorId), bytes(selectionRequestId), bytes(selectionRequestDigest),
    bytes(device.publicKey), selectionResult,
  ]);
  const selectionResponse = signed(DOMAINS.responseSignature, selectionContent, device);
  return {
    account_id_hex: core.expected.account_id_hex,
    device_id_hex: device.publicKey.toString("hex"),
    offer_id_hex: offerId.toString("hex"),
    request: requestJson(request),
    response: responseJson(response),
    selection_request: requestJson(selectionRequest),
    selection_response: responseJson(selectionResponse),
    context: {
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
      provider_properties_derive_from_signed_offer: true,
      mailbox_capabilities_in_payload: false,
      response_binds_complete_request_digest: true,
      active_runtime_changed: false,
    },
  };
}

function parse(argv) {
  const result = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--core-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const required of ["schema", "vectors", "core-vectors"])
    assert(result[required], `--${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
assert(schema.milestone === "M1.1.48" && schema.status === "candidate-not-active",
  "provider schema milestone or activation state changed");
const expected = generate(core, vectors.input);
equalJson(expected, vectors.expected, "canonical runtime IPC provider vectors");

if (options.selfTest) {
  const changed = structuredClone(vectors.input);
  changed.transport_identity_hex = `${changed.transport_identity_hex.slice(0, -2)}00`;
  let rejected = false;
  try { equalJson(generate(core, changed), vectors.expected, "changed transport identity"); }
  catch { rejected = true; }
  assert(rejected, "changed transport identity was unexpectedly accepted");
  const changedOffer = structuredClone(vectors.input);
  changedOffer.capacity_hint_bytes += 1;
  rejected = false;
  try { generate(core, changedOffer); } catch { rejected = true; }
  assert(rejected, "changed signed offer was unexpectedly accepted");
  const changedSelection = structuredClone(vectors.input);
  changedSelection.selection_salt_hex = `00${changedSelection.selection_salt_hex.slice(2)}`;
  rejected = false;
  try { equalJson(generate(core, changedSelection), vectors.expected, "changed selection salt"); }
  catch { rejected = true; }
  assert(rejected, "changed selection salt was unexpectedly accepted");
  console.log("canonical_runtime_ipc_provider_reference_self_test=passed");
}
console.log("canonical_runtime_ipc_provider_reference=verified");
console.log("independent_blake3=verified");
