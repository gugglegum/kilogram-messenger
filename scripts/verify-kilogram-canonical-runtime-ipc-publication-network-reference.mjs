import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3DeriveKey, blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const ROTATED_DERIVATION_CONTEXT =
  "kilogram ticket publication per-peer rotated write capability v1";
const DOMAINS = Object.fromEntries(Object.entries({
  channelId: "kilogram:ticket-publication-capability-channel:v1\0",
  peerScopeDigest: "kilogram:runtime-ipc-publication-peer-scope-digest:cbor-v1\0",
  publicationSignature: "kilogram:ticket-publication:cbor-v1\0",
  publicationId: "kilogram:ticket-publication-id:cbor-v1\0",
  ticketDigest: "kilogram:ticket-publication-ticket-digest:v1\0",
  observationSignature: "kilogram:ticket-publication-observation:cbor-v1\0",
  writeAuthorizationSignature:
    "kilogram:ticket-publication-write-authorization:cbor-v1\0",
  serviceOriginDigest:
    "kilogram:runtime-ipc-publication-network-service-origin-digest:cbor-v1\0",
  requestDigest:
    "kilogram:runtime-ipc-publication-network-request-digest:cbor-v1\0",
  responseSignature:
    "kilogram:runtime-ipc-publication-network-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-publication-network-response-id:cbor-v1\0",
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
const optionDigest = (value) => value === undefined
  ? array([uint(0)])
  : array([uint(1), bytes(value)]);
const publicationOptionalId = (value) => value === undefined
  ? array([])
  : array([bytes(value)]);

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
}
function u64be(value) {
  const output = Buffer.alloc(8); output.writeBigUInt64BE(BigInt(value)); return output;
}
function domainHash(domain, value) { return blake3Hash(Buffer.concat([domain, value])); }
function privateKey(seed) {
  return createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8",
  });
}
function publicBytes(key) {
  const der = createPublicKey(key).export({ format: "der", type: "spki" });
  assert(der.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI");
  return der.subarray(SPKI_PREFIX.length);
}
function publicFromBytes(raw) {
  return createPublicKey({ key: Buffer.concat([SPKI_PREFIX, raw]), format: "der", type: "spki" });
}
function sign(domain, content, signer, contentIsByteString) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer);
  const signerPublic = publicBytes(signer);
  assert(verifyEd25519(null, preimage, publicFromBytes(signerPublic), signature),
    "generated Ed25519 signature failed");
  const encodedContent = contentIsByteString ? bytes(content) : content;
  return {
    content,
    preimage,
    signature,
    cbor: array([uint(VERSION), encodedContent, bytes(signature)]),
  };
}
function capability(deviceSecret, peerScope, epoch) {
  assert(BigInt(epoch) > 0n, "rotated channel epoch must be positive");
  const seed = blake3DeriveKey(ROTATED_DERIVATION_CONTEXT,
    Buffer.concat([deviceSecret, peerScope, Buffer.from([0]), u64be(epoch)]));
  const key = privateKey(seed);
  const writeKey = publicBytes(key);
  const channelId = domainHash(DOMAINS.channelId, writeKey);
  return {
    key,
    writeKey,
    channelId,
    cbor: array([uint(VERSION), bytes(writeKey), bytes(channelId)]),
  };
}
function canonicalServiceOrigin(value) {
  const text = value.toString("utf8");
  assert(Buffer.from(text).equals(value), "service origin is not UTF-8");
  assert(value.length > 0 && value.length <= 512, "service origin length is outside bounds");
  const parsed = new URL(text);
  assert(parsed.protocol === "https:" && parsed.hostname.length > 0 &&
    parsed.username === "" && parsed.password === "" && parsed.pathname === "/" &&
    parsed.search === "" && parsed.hash === "" && parsed.href === text,
  "service origin must be an exact canonical root-only HTTPS URL");
  return value;
}
function requestJson(value) {
  const id = domainHash(DOMAINS.requestDigest, value);
  return {
    cbor_hex: value.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    id_preimage_hex: Buffer.concat([DOMAINS.requestDigest, value]).toString("hex"),
    object_id_hex: id.toString("hex"),
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
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((name) => [name, stable(value[name])]));
  }
  return value;
}
function equalJson(actual, expected, label) {
  const left = stable(actual); const right = stable(expected);
  if (JSON.stringify(left) === JSON.stringify(right)) return;
  const visit = (actualValue, expectedValue, path) => {
    if (JSON.stringify(actualValue) === JSON.stringify(expectedValue)) return undefined;
    if (actualValue !== null && expectedValue !== null &&
        typeof actualValue === "object" && typeof expectedValue === "object") {
      const names = [...new Set([...Object.keys(actualValue), ...Object.keys(expectedValue)])].sort();
      for (const name of names) {
        const mismatch = visit(actualValue[name], expectedValue[name], `${path}/${name}`);
        if (mismatch) return mismatch;
      }
    }
    return `${path}: actual=${JSON.stringify(actualValue)} expected=${JSON.stringify(expectedValue)}`;
  };
  fail(`${label} diverged at ${visit(left, right, "expected")}`);
}

function publication({ signer, channelId, publisherAccount, recipientAccount,
  ticket, publishedAt, ttl }) {
  assert(ttl >= 30 && ttl <= 3600, "ticket publication TTL is outside bounds");
  const publisherDevice = publicBytes(signer);
  const ticketDigest = domainHash(DOMAINS.ticketDigest, ticket);
  const content = array([
    uint(VERSION), bytes(channelId), bytes(publisherAccount), bytes(publisherDevice),
    bytes(recipientAccount), uint(1), publicationOptionalId(undefined), uint(publishedAt),
    uint(publishedAt + ttl), bytes(ticketDigest), bytes(ticket),
  ]);
  const signed = sign(DOMAINS.publicationSignature, content, signer, true);
  return {
    ...signed,
    id: domainHash(DOMAINS.publicationId, signed.cbor),
    ticketDigest,
    generation: 1,
    publishedAt,
    expiresAt: publishedAt + ttl,
    publisherDevice,
  };
}

function generate(core, input) {
  const deviceSecret = fromHex(input.device_secret_hex, 32, "Device secret");
  const deviceSigner = privateKey(deviceSecret);
  const devicePublic = publicBytes(deviceSigner);
  assert(devicePublic.toString("hex") === core.expected.device_id_hex,
    "core Device dependency changed");
  const localAccount = fromHex(core.expected.account_id_hex, 32, "local Account ID");
  const descriptorId = fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID");
  const bearer = fromHex(core.input.bearer_token_hex, 32, "bearer token");
  const peerAccount = fromHex(input.peer_account_id_hex, 32, "peer Account ID");
  const origin = canonicalServiceOrigin(fromHex(input.service_origin_utf8_hex, undefined,
    "service origin"));

  const peerScope = fromHex(input.peer_scope_hex, undefined, "publish peer scope");
  const publishCapability = capability(deviceSecret, peerScope, input.channel_epoch);
  const publishContact = fromHex(input.publish_contact_id_hex, 32, "publish contact ID");
  const recipientDigest = fromHex(input.recipient_device_list_digest_hex, 32,
    "recipient Device-list digest");
  assert(input.recipient_device_count >= 1 && input.recipient_device_count <= 32,
    "recipient Device count is outside bounds");
  const peerScopeDigest = domainHash(DOMAINS.peerScopeDigest, peerScope);
  const channelState = array([
    uint(VERSION), bytes(publishContact), bytes(peerAccount), bytes(peerScopeDigest),
    uint(input.channel_epoch), bytes(publishCapability.cbor), optionDigest(undefined), uint(0),
  ]);
  const publishState = array([
    uint(VERSION), bytes(channelState), bytes(recipientDigest),
    uint(input.recipient_device_count),
  ]);
  const publishRequestId = fromHex(input.publish_request_id_hex, 32, "publish request ID");
  const publishCommand = array([
    uint(0), bytes(publishState), bytes(origin), uint(input.publish_ttl_seconds),
  ]);
  const publishRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(publishRequestId), bytes(bearer),
    uint(input.publish_request_issued_at_unix_seconds),
    uint(input.publish_request_expires_at_unix_seconds), publishCommand,
  ]);
  const publishRequestDigest = domainHash(DOMAINS.requestDigest, publishRequest);
  const publishTicket = fromHex(input.publish_ticket_utf8_hex, undefined, "publish ticket");
  const innerPublication = publication({
    signer: deviceSigner,
    channelId: publishCapability.channelId,
    publisherAccount: localAccount,
    recipientAccount: peerAccount,
    ticket: publishTicket,
    publishedAt: input.publish_at_unix_seconds,
    ttl: input.publish_ttl_seconds,
  });
  const opaqueEnvelope = fromHex(input.opaque_envelope_fixture_hex, undefined,
    "opaque envelope fixture");
  const envelopeDigest = blake3Hash(opaqueEnvelope);
  const authorizationContent = array([
    uint(VERSION), bytes(publishCapability.cbor), uint(1), uint(opaqueEnvelope.length),
    bytes(envelopeDigest),
  ]);
  const authorization = sign(DOMAINS.writeAuthorizationSignature,
    authorizationContent, publishCapability.key, true);
  const updatedChannel = array([
    uint(VERSION), bytes(publishContact), bytes(peerAccount), bytes(peerScopeDigest),
    uint(input.channel_epoch), bytes(publishCapability.cbor),
    optionDigest(innerPublication.id), uint(1),
  ]);
  const publishResult = array([
    uint(VERSION), uint(0), bytes(updatedChannel), uint(innerPublication.publishedAt),
    uint(innerPublication.expiresAt), bytes(innerPublication.ticketDigest),
    bytes(recipientDigest), uint(input.recipient_device_count), uint(opaqueEnvelope.length),
    bytes(envelopeDigest), bytes(authorization.cbor), bytes(origin),
    uint(0), uint(0), uint(0), uint(0),
  ]);
  const publishResponseContent = array([
    uint(VERSION), bytes(descriptorId), bytes(publishRequestId), bytes(publishRequestDigest),
    bytes(devicePublic), array([uint(0), bytes(publishResult)]),
  ]);
  const publishResponse = sign(DOMAINS.responseSignature, publishResponseContent,
    deviceSigner, false);

  const peerDeviceSecret = fromHex(input.peer_device_secret_hex, 32, "peer Device secret");
  const peerDeviceSigner = privateKey(peerDeviceSecret);
  const refreshPeerScope = fromHex(input.refresh_peer_scope_hex, undefined,
    "refresh peer scope");
  const refreshCapability = capability(peerDeviceSecret, refreshPeerScope,
    input.refresh_channel_epoch);
  const refreshTicket = fromHex(input.refresh_ticket_utf8_hex, undefined, "refresh ticket");
  const fetchedPublication = publication({
    signer: peerDeviceSigner,
    channelId: refreshCapability.channelId,
    publisherAccount: peerAccount,
    recipientAccount: localAccount,
    ticket: refreshTicket,
    publishedAt: input.refresh_publication_at_unix_seconds,
    ttl: input.refresh_publication_ttl_seconds,
  });
  const refreshContact = fromHex(input.refresh_contact_id_hex, 32, "refresh contact ID");
  const refreshState = array([
    uint(VERSION), bytes(refreshContact), bytes(peerAccount),
    bytes(fetchedPublication.publisherDevice), bytes(refreshCapability.channelId),
    optionDigest(undefined), uint(0), optionDigest(undefined), uint(0),
  ]);
  const refreshRequestId = fromHex(input.refresh_request_id_hex, 32, "refresh request ID");
  const refreshCommand = array([uint(1), bytes(refreshState), bytes(origin)]);
  const refreshRequest = array([
    uint(VERSION), bytes(descriptorId), bytes(refreshRequestId), bytes(bearer),
    uint(input.refresh_request_issued_at_unix_seconds),
    uint(input.refresh_request_expires_at_unix_seconds), refreshCommand,
  ]);
  const refreshRequestDigest = domainHash(DOMAINS.requestDigest, refreshRequest);
  const observationContent = array([
    uint(VERSION), bytes(localAccount), bytes(devicePublic), bytes(refreshCapability.channelId),
    bytes(peerAccount), bytes(fetchedPublication.publisherDevice), uint(1),
    publicationOptionalId(undefined), uint(fetchedPublication.generation),
    bytes(fetchedPublication.id),
    bytes(fetchedPublication.ticketDigest), uint(input.refresh_observed_at_unix_seconds),
  ]);
  const observation = sign(DOMAINS.observationSignature, observationContent,
    deviceSigner, true);
  const refreshResult = array([
    uint(VERSION), uint(0), bytes(refreshContact), bytes(peerAccount),
    bytes(fetchedPublication.publisherDevice), bytes(refreshCapability.channelId),
    bytes(observation.cbor), uint(fetchedPublication.expiresAt), bytes(origin),
    uint(0), uint(0), uint(0), uint(0),
  ]);
  const refreshResponseContent = array([
    uint(VERSION), bytes(descriptorId), bytes(refreshRequestId), bytes(refreshRequestDigest),
    bytes(devicePublic), array([uint(1), bytes(refreshResult)]),
  ]);
  const refreshResponse = sign(DOMAINS.responseSignature, refreshResponseContent,
    deviceSigner, false);

  return {
    account_id_hex: localAccount.toString("hex"),
    device_id_hex: devicePublic.toString("hex"),
    service_origin_digest_hex: domainHash(DOMAINS.serviceOriginDigest, origin).toString("hex"),
    publish: {
      inner_publication_fixture_signed_cbor_hex: innerPublication.cbor.toString("hex"),
      state_cbor_hex: publishState.toString("hex"),
      result_cbor_hex: publishResult.toString("hex"),
      request: requestJson(publishRequest),
      response: responseJson(publishResponse),
    },
    refresh: {
      inner_publication_fixture_signed_cbor_hex: fetchedPublication.cbor.toString("hex"),
      state_cbor_hex: refreshState.toString("hex"),
      result_cbor_hex: refreshResult.toString("hex"),
      request: requestJson(refreshRequest),
      response: responseJson(refreshResponse),
    },
    context: {
      active_runtime_changed: false,
      network_io_executed: false,
      hpke_envelope_fixture_only: true,
      ticket_plaintext_in_ipc: false,
      hpke_ciphertext_in_ipc: false,
      write_secret_in_ipc: false,
      https_success_is_device_attestation_not_remote_durability_proof: true,
      publish_delivery_requires_separate_peer_observation: true,
      refresh_install_is_atomic_before_success: true,
    },
  };
}

function parse(argv) {
  const result = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (["--schema", "--vectors", "--core-vectors", "--contact-vectors",
      "--ticket-vectors", "--conflict-vectors"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  for (const required of ["schema", "vectors", "core-vectors", "contact-vectors",
    "ticket-vectors", "conflict-vectors"]) assert(result[required], `--${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
const contact = JSON.parse(readFileSync(options["contact-vectors"], "utf8"));
const ticket = JSON.parse(readFileSync(options["ticket-vectors"], "utf8"));
const conflict = JSON.parse(readFileSync(options["conflict-vectors"], "utf8"));
assert(schema.milestone === "M1.1.41" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC publication-network schema header changed");
assert(schema.limits.maximum_publication_network_request_bytes === 131072 &&
  schema.limits.maximum_publication_network_response_bytes === 131072 &&
  schema.limits.maximum_publication_service_origin_bytes === 512 &&
  schema.limits.maximum_publication_recipient_devices === 32,
"runtime IPC publication-network bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v26-with-json-descriptor-v7" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "publication-network vector header changed");
assert(vectors.input.peer_account_id_hex === contact.input.peer_account_id_hex,
  "contact dependency peer Account changed");
assert(ticket.expected.domains.rotated_derivation_context === ROTATED_DERIVATION_CONTEXT,
  "ticket publication derivation context changed");
assert(conflict.expected.first_publication !== undefined,
  "publication-conflict dependency changed");
equalJson(generate(core, vectors.input), vectors.expected,
  "independent canonical runtime IPC publication-network vector");

if (options.selfTest) {
  const changedOrigin = structuredClone(vectors.input);
  changedOrigin.service_origin_utf8_hex = Buffer.from("https://other.example/").toString("hex");
  assert(generate(core, changedOrigin).publish.request.object_id_hex !==
    vectors.expected.publish.request.object_id_hex,
  "changed service origin preserved publish request digest");
  const changedEnvelope = structuredClone(vectors.input);
  changedEnvelope.opaque_envelope_fixture_hex =
    `ff${changedEnvelope.opaque_envelope_fixture_hex.slice(2)}`;
  assert(generate(core, changedEnvelope).publish.result_cbor_hex !==
    vectors.expected.publish.result_cbor_hex,
  "changed opaque envelope preserved publish result");
  const changedPeerDevice = structuredClone(vectors.input);
  changedPeerDevice.peer_device_secret_hex =
    `3f${changedPeerDevice.peer_device_secret_hex.slice(2)}`;
  assert(generate(core, changedPeerDevice).refresh.result_cbor_hex !==
    vectors.expected.refresh.result_cbor_hex,
  "changed peer Device preserved refresh result");
  const invalidOrigin = structuredClone(vectors.input);
  invalidOrigin.service_origin_utf8_hex = Buffer.from("http://tickets.example/").toString("hex");
  let rejected = false;
  try { generate(core, invalidOrigin); } catch { rejected = true; }
  assert(rejected, "non-HTTPS service origin was accepted");
  process.stdout.write("changed_publication_network_origin=rejected\n");
  process.stdout.write("changed_publication_network_envelope=rejected\n");
  process.stdout.write("changed_publication_network_peer_device=rejected\n");
  process.stdout.write("non_https_publication_network_origin=rejected\n");
  process.stdout.write("canonical_runtime_ipc_publication_network_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_publication_network_reference=verified\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("independent_blake3_hash_and_derive_key=verified\n");
  process.stdout.write("ticket_plaintext_in_ipc=false\n");
  process.stdout.write("hpke_ciphertext_in_ipc=false\n");
  process.stdout.write("write_secret_in_ipc=false\n");
  process.stdout.write("network_executed=false\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
