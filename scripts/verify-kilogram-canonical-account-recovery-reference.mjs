import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  deviceCertificate: "kilogram:device-certificate-signature:cbor-v1\0",
  authoritySnapshot: "kilogram:account-authority-snapshot-signature:cbor-v1\0",
  deviceList: "kilogram:account-device-list-signature:cbor-v1\0",
  membership: "kilogram:conversation-membership-signature:cbor-v1\0",
  packageSignature: "kilogram:account-root-recovery-package-signature:cbor-v1\0",
  packageId: "kilogram:account-root-recovery-package-id:cbor-v1\0",
  stateVector: "kilogram:account-root-recovery-state-vector:cbor-v1\0",
  roster: "kilogram:account-root-recovery-voter-roster:cbor-v1\0",
  witnessSignature: "kilogram:account-root-recovery-witness-signature:cbor-v1\0",
  requestId: "kilogram:account-root-recovery-approval-request-id:cbor-v1\0",
  approvalSignature: "kilogram:account-root-recovery-approval-signature:cbor-v1\0",
  approvalId: "kilogram:account-root-recovery-approval-id:cbor-v1\0",
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
function signed(domain, content, signer) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer.privateKey);
  assert(verifyEd25519(null, preimage, publicFromBytes(signer.publicKey), signature),
    "generated Ed25519 signature failed");
  return {
    content,
    preimage,
    signature,
    cbor: array([uint(VERSION), content, bytes(signature)]),
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
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}

function verifySignedFixture(name, object, domain, publicKey) {
  const content = fromHex(object.content_cbor_hex, undefined, `${name} content`);
  const signature = fromHex(object.signature_hex, 64, `${name} signature`);
  const preimage = Buffer.concat([domain, content]);
  assert(preimage.toString("hex") === object.signing_preimage_hex, `${name} preimage diverged`);
  assert(verifyEd25519(null, preimage, publicFromBytes(publicKey), signature),
    `${name} signature failed`);
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  assert(envelope.toString("hex") === object.signed_cbor_hex, `${name} envelope diverged`);
  return envelope;
}

function dependencies(authority, account) {
  const root = key(authority.input.root_secret_key_hex, "Root seed");
  const device = key(authority.input.device_secret_key_hex, "Device seed");
  assert(root.publicKey.toString("hex") === authority.expected.account_id_hex,
    "authority Account ID diverged");
  assert(device.publicKey.toString("hex") === authority.expected.device_id_hex,
    "authority Device ID diverged");
  const snapshot = verifySignedFixture(
    "authority snapshot", authority.expected.authority_snapshot,
    DOMAINS.authoritySnapshot, root.publicKey,
  );
  const certificate = verifySignedFixture(
    "device certificate", authority.expected.certificate,
    DOMAINS.deviceCertificate, root.publicKey,
  );
  const deviceListContent = array([
    uint(VERSION), bytes(snapshot), array([bytes(certificate)]),
  ]);
  const deviceList = signed(DOMAINS.deviceList, deviceListContent, root);
  assert(deviceList.cbor.toString("hex") === account.expected.account_device_list.signed_cbor_hex,
    "account device list diverged");

  const other = key(account.input.other_member_secret_key_hex, "other Account seed");
  const members = [root.publicKey, other.publicKey].sort(Buffer.compare);
  const membershipContent = array([
    uint(VERSION), bytes(fromHex(account.input.conversation_id_hex, 32, "conversation ID")),
    uint(account.input.membership_revision), bytes(root.publicKey),
    array(members.map(bytes)),
  ]);
  const membership = signed(DOMAINS.membership, membershipContent, root);
  assert(membership.cbor.toString("hex") ===
    account.expected.conversation_membership.signed_cbor_hex,
  "conversation membership diverged");
  return { root, device, snapshot, deviceList: deviceList.cbor, membership: membership.cbor };
}

function generate(authority, account, input) {
  const dependency = dependencies(authority, account);
  const authorityRevision = authority.input.authority_revision;
  const packageContent = array([
    uint(VERSION), bytes(dependency.root.publicKey), bytes(dependency.snapshot),
    bytes(dependency.deviceList), array([bytes(dependency.membership)]),
  ]);
  const packageEnvelope = signed(DOMAINS.packageSignature, packageContent, dependency.root);
  const packageId = domainHash(DOMAINS.packageId, packageEnvelope.cbor);
  const stateVectorInput = array([
    uint(VERSION), bytes(dependency.snapshot), bytes(dependency.deviceList),
    array([bytes(dependency.membership)]),
  ]);
  const stateVectorDigest = domainHash(DOMAINS.stateVector, stateVectorInput);
  const rosterDigest = domainHash(DOMAINS.roster, dependency.deviceList);
  const witnessContent = array([
    uint(VERSION), bytes(dependency.root.publicKey), uint(authorityRevision), bytes(packageId),
  ]);
  const witness = signed(DOMAINS.witnessSignature, witnessContent, dependency.root);
  const challenge = fromHex(input.challenge_hex, 32, "challenge");
  const request = array([
    uint(VERSION), bytes(packageEnvelope.cbor), bytes(witness.cbor), bytes(challenge),
    uint(input.issued_at_unix_seconds), uint(input.expires_at_unix_seconds),
  ]);
  const requestId = domainHash(DOMAINS.requestId, request);
  const previousHead = fromHex(input.previous_approval_head_digest_hex, 32,
    "previous approval head");
  const approvalContent = array([
    uint(VERSION), bytes(dependency.root.publicKey), bytes(requestId), bytes(packageId),
    uint(authorityRevision), bytes(stateVectorDigest), bytes(rosterDigest), bytes(challenge),
    uint(input.expires_at_unix_seconds), bytes(dependency.device.publicKey), bytes(previousHead),
    uint(input.approved_at_unix_seconds),
  ]);
  const approval = signed(DOMAINS.approvalSignature, approvalContent, dependency.device);
  const approvalId = domainHash(DOMAINS.approvalId, approval.cbor);
  return {
    account_id_hex: dependency.root.publicKey.toString("hex"),
    approver_device_id_hex: dependency.device.publicKey.toString("hex"),
    dependencies: {
      authority_snapshot_cbor_hex: dependency.snapshot.toString("hex"),
      account_device_list_cbor_hex: dependency.deviceList.toString("hex"),
      conversation_membership_cbor_hex: dependency.membership.toString("hex"),
    },
    package: {
      ...signedJson(packageEnvelope),
      id_preimage_hex: Buffer.concat([DOMAINS.packageId, packageEnvelope.cbor]).toString("hex"),
      object_id_hex: packageId.toString("hex"),
    },
    state_vector: {
      input_cbor_hex: stateVectorInput.toString("hex"),
      preimage_hex: Buffer.concat([DOMAINS.stateVector, stateVectorInput]).toString("hex"),
      digest_hex: stateVectorDigest.toString("hex"),
    },
    roster: {
      input_cbor_hex: dependency.deviceList.toString("hex"),
      preimage_hex: Buffer.concat([DOMAINS.roster, dependency.deviceList]).toString("hex"),
      digest_hex: rosterDigest.toString("hex"),
    },
    witness: signedJson(witness),
    request: {
      cbor_hex: request.toString("hex"),
      id_preimage_hex: Buffer.concat([DOMAINS.requestId, request]).toString("hex"),
      object_id_hex: requestId.toString("hex"),
    },
    approval: {
      ...signedJson(approval),
      id_preimage_hex: Buffer.concat([DOMAINS.approvalId, approval.cbor]).toString("hex"),
      object_id_hex: approvalId.toString("hex"),
    },
    quorum: {
      roster_count: 1,
      observed_approvals: 1,
      required_approvals: 1,
      claim: "current-device-majority-observed",
    },
  };
}

function parse(argv) {
  const result = { selfTest: false, emit: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (argument === "--emit") result.emit = true;
    else if (["--schema", "--vectors", "--authority-vectors", "--account-vectors", "--output"]
      .includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["authority-vectors"], "--authority-vectors is required");
  assert(result["account-vectors"], "--account-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const options = parse(process.argv.slice(2));
const authority = JSON.parse(readFileSync(options["authority-vectors"], "utf8"));
const account = JSON.parse(readFileSync(options["account-vectors"], "utf8"));
const defaultInput = {
  challenge_hex: "e0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
  issued_at_unix_seconds: 1800000000,
  expires_at_unix_seconds: 1800000600,
  previous_approval_head_digest_hex:
    "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff000102030405060708090a0b0c0d0e0f",
  approved_at_unix_seconds: 1800000005,
};
if (options.emit) {
  const document = {
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(authority, account, defaultInput),
  };
  const output = `${JSON.stringify(document, null, 2)}\n`;
  if (options.output) writeFileSync(options.output, output);
  else process.stdout.write(output);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.26" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "recovery schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "recovery vector header changed");
equalJson(generate(authority, account, vectors.input), vectors.expected,
  "independent Account recovery vector");
if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.expected.approval.signature_hex =
    `00${changed.expected.approval.signature_hex.slice(2)}`;
  let rejected = false;
  try {
    equalJson(generate(authority, account, changed.input), changed.expected, "tampered vector");
  } catch {
    rejected = true;
  }
  assert(rejected, "tampered approval vector was accepted");
  process.stdout.write("canonical_account_recovery_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_account_recovery_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519-and-javascript-blake3\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("active_wire_changed=false\n");
}
