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
const DOMAINS = {
  certificate: Buffer.from("kilogram:device-certificate-signature:cbor-v1\0"),
  snapshot: Buffer.from("kilogram:account-authority-snapshot-signature:cbor-v1\0"),
  deviceList: Buffer.from("kilogram:account-device-list-signature:cbor-v1\0"),
  sas: Buffer.from("kilogram:history-recovery-sas:cbor-v1\0"),
  linkSignature: Buffer.from("kilogram:history-recovery-link-signature:cbor-v1\0"),
  linkId: Buffer.from("kilogram:history-recovery-link-id:cbor-v1\0"),
  planSignature: Buffer.from("kilogram:history-recovery-plan-signature:cbor-v1\0"),
  planId: Buffer.from("kilogram:history-recovery-plan-id:cbor-v1\0"),
  recoveryId: Buffer.from("kilogram:history-recovery-id:cbor-v1\0"),
  requestSignature: Buffer.from("kilogram:history-recovery-page-request-signature:cbor-v1\0"),
  responseSignature: Buffer.from("kilogram:history-recovery-response-signature:cbor-v1\0"),
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
function privateFromSeed(seed) {
  return createPrivateKey({ key: Buffer.concat([PKCS8_PREFIX, seed]), format: "der", type: "pkcs8" });
}
function publicBytes(privateKey) {
  const spki = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX), "unexpected Ed25519 SPKI prefix");
  return spki.subarray(SPKI_PREFIX.length);
}
function signed(content, domain, privateKey, idDomain) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, privateKey);
  assert(verifyEd25519(null, preimage, createPublicKey(privateKey), signature), "self signature failed");
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  return {
    content,
    domain,
    preimage,
    signature,
    envelope,
    idDomain,
    id: idDomain === undefined ? undefined : blake3Hash(Buffer.concat([idDomain, envelope])),
  };
}
function expectedSigned(value) {
  const result = {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: value.domain.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
  };
  if (value.id !== undefined) {
    result.id_domain_hex = value.idDomain.toString("hex");
    result.id_hex = value.id.toString("hex");
  }
  return result;
}
function certificate(account, device, encryption, sequence, rootKey) {
  const content = array([
    uint(VERSION), bytes(account), bytes(device), bytes(encryption), uint(sequence),
    array([uint(1), uint(2)]),
  ]);
  return signed(content, DOMAINS.certificate, rootKey);
}
function policy(values) {
  return array([
    uint(VERSION), uint(values.ethernet ? 1 : 0), uint(values.wifi ? 1 : 0),
    uint(values.mobile ? 1 : 0), uint(values.unknown ? 1 : 0),
    uint(values.externalPower ? 1 : 0),
  ]);
}

function generate(input) {
  const rootKey = privateFromSeed(fromHex(input.root_secret_key_hex, 32, "Root seed"));
  const sourceKey = privateFromSeed(fromHex(input.source_device_secret_key_hex, 32, "source seed"));
  const recipientKey = privateFromSeed(fromHex(input.recipient_device_secret_key_hex, 32, "recipient seed"));
  const account = publicBytes(rootKey);
  const source = publicBytes(sourceKey);
  const recipient = publicBytes(recipientKey);
  const sourceCertificate = certificate(account, source,
    fromHex(input.source_encryption_public_key_hex, 32, "source encryption key"), 1, rootKey);
  const recipientCertificate = certificate(account, recipient,
    fromHex(input.recipient_encryption_public_key_hex, 32, "recipient encryption key"), 2, rootKey);
  const snapshotContent = array([uint(VERSION), bytes(account), uint(3), array([])]);
  const snapshot = signed(snapshotContent, DOMAINS.snapshot, rootKey);
  const certificates = [
    { device: source, certificate: sourceCertificate },
    { device: recipient, certificate: recipientCertificate },
  ].sort((left, right) => left.device.compare(right.device));
  const deviceListContent = array([
    uint(VERSION), bytes(snapshot.envelope),
    array(certificates.map((value) => bytes(value.certificate.envelope))),
  ]);
  const deviceList = signed(deviceListContent, DOMAINS.deviceList, rootKey);

  const conversation = fromHex(input.conversation_id_hex, 32, "conversation ID");
  const endpoint = fromHex(input.endpoint_id_hex, 32, "endpoint ID");
  const coordinates = [
    [1, Buffer.from(input.relay_url, "utf8")],
    [2, Buffer.from(input.direct_address, "utf8")],
  ];
  const linkContent = array([
    uint(VERSION), bytes(endpoint),
    array(coordinates.map(([kind, value]) => array([uint(kind), bytes(value)]))),
    bytes(sourceCertificate.envelope), bytes(deviceList.envelope), bytes(recipient),
    bytes(conversation), uint(input.approved_range_start), uint(input.approved_range_end),
    uint(input.page_size), uint(input.route_policy), uint(input.link_issued_at),
    uint(input.link_expires_at),
  ]);
  const link = signed(linkContent, DOMAINS.linkSignature, sourceKey, DOMAINS.linkId);

  const sasInput = array([
    uint(VERSION), bytes(deviceList.envelope), bytes(source), bytes(recipient),
  ]);
  const sas = blake3Hash(Buffer.concat([DOMAINS.sas, sasInput]));
  const executionPolicy = policy({ ethernet: true, wifi: true, mobile: false, unknown: false, externalPower: false });
  const planContent = array([
    uint(VERSION), bytes(link.envelope), bytes(sas), executionPolicy,
    uint(input.plan_approved_at), uint(input.plan_expires_at),
  ]);
  const plan = signed(planContent, DOMAINS.planSignature, recipientKey, DOMAINS.planId);

  const recoveryIdentity = array([
    uint(VERSION), bytes(account), bytes(conversation), bytes(source), bytes(recipient), bytes(sas),
    uint(input.approved_range_start), uint(input.approved_range_end), uint(input.page_size),
  ]);
  const recoveryId = blake3Hash(Buffer.concat([DOMAINS.recoveryId, recoveryIdentity]));
  const requestContent = array([
    uint(VERSION), bytes(plan.id), bytes(recoveryId), bytes(conversation), bytes(source),
    bytes(recipient), bytes(fromHex(input.session_binding_hex, 32, "session binding")),
    uint(input.approved_range_start), uint(input.page_size), bytes(sas), array([]),
  ]);
  const request = signed(requestContent, DOMAINS.requestSignature, recipientKey);

  const pageBody = array([
    uint(VERSION), bytes(request.envelope), uint(input.inventory_event_count),
    bytes(fromHex(input.inventory_digest_hex, 32, "inventory digest")),
    uint(input.approved_range_start), uint(input.page_range_end),
    bytes(fromHex(input.opaque_rewrapped_payload_hex, undefined, "opaque payload")),
  ]);
  const pageContent = array([uint(VERSION), uint(1), bytes(pageBody)]);
  const pageResponse = signed(pageContent, DOMAINS.responseSignature, sourceKey);

  const rejectedBody = array([uint(VERSION), bytes(request.envelope), uint(2)]);
  const rejectedContent = array([uint(VERSION), uint(2), bytes(rejectedBody)]);
  const rejectedResponse = signed(rejectedContent, DOMAINS.responseSignature, sourceKey);

  return {
    account_id_hex: account.toString("hex"),
    source_device_id_hex: source.toString("hex"),
    recipient_device_id_hex: recipient.toString("hex"),
    source_certificate_cbor_hex: sourceCertificate.envelope.toString("hex"),
    recipient_certificate_cbor_hex: recipientCertificate.envelope.toString("hex"),
    account_device_list_cbor_hex: deviceList.envelope.toString("hex"),
    sas_input_cbor_hex: sasInput.toString("hex"),
    sas_domain_hex: DOMAINS.sas.toString("hex"),
    sas_hex: sas.toString("hex"),
    link: expectedSigned(link),
    execution_policy_cbor_hex: executionPolicy.toString("hex"),
    plan: expectedSigned(plan),
    recovery_identity_cbor_hex: recoveryIdentity.toString("hex"),
    recovery_id_domain_hex: DOMAINS.recoveryId.toString("hex"),
    recovery_id_hex: recoveryId.toString("hex"),
    request: expectedSigned(request),
    page_response: { page_cbor_hex: pageBody.toString("hex"), ...expectedSigned(pageResponse) },
    rejected_response: { rejected_cbor_hex: rejectedBody.toString("hex"), ...expectedSigned(rejectedResponse) },
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
  if (!result.emit) { assert(result.schema, "--schema is required"); assert(result.vectors, "--vectors is required"); }
  return result;
}

const defaultInput = {
  root_secret_key_hex: "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
  source_device_secret_key_hex: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  recipient_device_secret_key_hex: "4142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f60",
  source_encryption_public_key_hex: "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  recipient_encryption_public_key_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  conversation_id_hex: "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeef",
  endpoint_id_hex: "e0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
  relay_url: "https://euc1-1.relay.n0.iroh.link./",
  direct_address: "203.0.113.7:4242",
  approved_range_start: 100,
  approved_range_end: 120,
  page_size: 8,
  route_policy: 1,
  link_issued_at: 1800000000,
  link_expires_at: 1800000600,
  plan_approved_at: 1800000060,
  plan_expires_at: 1800086460,
  session_binding_hex: "f0e0d0c0b0a09080706050403020100000102030405060708090a0b0c0d0e0f0",
  inventory_event_count: 120,
  inventory_digest_hex: "c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0",
  page_range_end: 108,
  opaque_rewrapped_payload_hex: "0102030405060708090a0b0c0d0e0f10",
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
assert(schema.milestone === "M1.1.14" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "recovery exchange schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "recovery exchange vector header changed");
equalJson(generate(vectors.input), vectors.expected, "independent recovery exchange vector");

if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.input.recipient_device_secret_key_hex = `ff${changed.input.recipient_device_secret_key_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(changed.input), changed.expected, "changed recipient key"); } catch { rejected = true; }
  assert(rejected, "changed recipient key was accepted");
  process.stdout.write("canonical_recovery_exchange_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_recovery_exchange_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_ed25519=verified\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("active_runtime_changed=false\n");
}
