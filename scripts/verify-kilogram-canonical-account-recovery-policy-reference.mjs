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
  roster: "kilogram:account-root-recovery-voter-roster:cbor-v1\0",
  witnessSignature: "kilogram:account-root-recovery-witness-signature:cbor-v1\0",
  policyRequestId: "kilogram:account-recovery-policy-transition-request-id:cbor-v1\0",
  policyApprovalSignature:
    "kilogram:account-recovery-policy-transition-approval-signature:cbor-v1\0",
  policyApprovalId: "kilogram:account-recovery-policy-transition-approval-id:cbor-v1\0",
  policyCertificateId:
    "kilogram:account-recovery-policy-transition-certificate-id:cbor-v1\0",
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
  return { content, preimage, signature, cbor: array([uint(VERSION), content, bytes(signature)]) };
}
function signedJson(value, idDomain) {
  const output = {
    content_cbor_hex: value.content.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
  };
  if (idDomain) {
    const id = domainHash(idDomain, value.cbor);
    output.id_preimage_hex = Buffer.concat([idDomain, value.cbor]).toString("hex");
    output.object_id_hex = id.toString("hex");
  }
  return output;
}
function objectJson(value, idDomain) {
  const id = domainHash(idDomain, value);
  return {
    cbor_hex: value.toString("hex"),
    id_preimage_hex: Buffer.concat([idDomain, value]).toString("hex"),
    object_id_hex: id.toString("hex"),
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

function generate(authority, account, recovery, input) {
  const root = key(authority.input.root_secret_key_hex, "Root seed");
  const oldDevice = key(authority.input.device_secret_key_hex, "old Device seed");
  const newDevice = key(input.new_device_secret_key_hex, "new Device seed");
  assert(root.publicKey.toString("hex") === authority.expected.account_id_hex,
    "Account ID diverged");
  assert(oldDevice.publicKey.toString("hex") === authority.expected.device_id_hex,
    "old Device ID diverged");

  const oldSnapshot = verifySignedFixture(
    "old authority snapshot", authority.expected.authority_snapshot,
    DOMAINS.authoritySnapshot, root.publicKey,
  );
  const oldCertificate = verifySignedFixture(
    "old Device certificate", authority.expected.certificate,
    DOMAINS.deviceCertificate, root.publicKey,
  );
  const oldRevocation = verifySignedFixture(
    "old revocation", authority.expected.revocation,
    Buffer.from("kilogram:device-revocation-signature:cbor-v1\0"), root.publicKey,
  );
  const membership = verifySignedFixture(
    "conversation membership", account.expected.conversation_membership,
    DOMAINS.membership, root.publicKey,
  );
  const oldDeviceListContent = array([
    uint(VERSION), bytes(oldSnapshot), array([bytes(oldCertificate)]),
  ]);
  const oldDeviceList = signed(DOMAINS.deviceList, oldDeviceListContent, root);
  assert(oldDeviceList.cbor.toString("hex") ===
    recovery.expected.dependencies.account_device_list_cbor_hex,
  "old Device list diverged");
  const oldPackageContent = array([
    uint(VERSION), bytes(root.publicKey), bytes(oldSnapshot), bytes(oldDeviceList.cbor),
    array([bytes(membership)]),
  ]);
  const oldPackage = signed(DOMAINS.packageSignature, oldPackageContent, root);
  assert(oldPackage.cbor.toString("hex") === recovery.expected.package.signed_cbor_hex,
    "old recovery package diverged");
  const oldPackageId = domainHash(DOMAINS.packageId, oldPackage.cbor);
  const oldWitnessContent = array([
    uint(VERSION), bytes(root.publicKey), uint(authority.input.authority_revision),
    bytes(oldPackageId),
  ]);
  const oldWitness = signed(DOMAINS.witnessSignature, oldWitnessContent, root);
  assert(oldWitness.cbor.toString("hex") === recovery.expected.witness.signed_cbor_hex,
    "old recovery witness diverged");

  const newCertificateContent = array([
    uint(VERSION), bytes(root.publicKey), bytes(newDevice.publicKey),
    bytes(fromHex(input.new_device_encryption_public_key_hex, 32, "new encryption key")),
    uint(input.new_certificate_authority_sequence), array([uint(1), uint(2)]),
  ]);
  const newCertificate = signed(DOMAINS.deviceCertificate, newCertificateContent, root);
  const newSnapshotContent = array([
    uint(VERSION), bytes(root.publicKey), uint(input.new_authority_revision),
    array([oldRevocation]),
  ]);
  const newSnapshot = signed(DOMAINS.authoritySnapshot, newSnapshotContent, root);
  const orderedCertificates = [
    { id: oldDevice.publicKey, cbor: oldCertificate },
    { id: newDevice.publicKey, cbor: newCertificate.cbor },
  ].sort((left, right) => Buffer.compare(left.id, right.id));
  const newDeviceListContent = array([
    uint(VERSION), bytes(newSnapshot.cbor),
    array(orderedCertificates.map((certificate) => bytes(certificate.cbor))),
  ]);
  const newDeviceList = signed(DOMAINS.deviceList, newDeviceListContent, root);
  const newPackageContent = array([
    uint(VERSION), bytes(root.publicKey), bytes(newSnapshot.cbor), bytes(newDeviceList.cbor),
    array([bytes(membership)]),
  ]);
  const newPackage = signed(DOMAINS.packageSignature, newPackageContent, root);
  const newPackageId = domainHash(DOMAINS.packageId, newPackage.cbor);
  const newWitnessContent = array([
    uint(VERSION), bytes(root.publicKey), uint(input.new_authority_revision), bytes(newPackageId),
  ]);
  const newWitness = signed(DOMAINS.witnessSignature, newWitnessContent, root);

  const oldRosterDigest = domainHash(DOMAINS.roster, oldDeviceList.cbor);
  const newRosterDigest = domainHash(DOMAINS.roster, newDeviceList.cbor);
  assert(!oldRosterDigest.equals(newRosterDigest), "recovery roster did not change");
  const previousTransitionId = fromHex(input.previous_transition_id_hex, 32,
    "previous transition ID");
  const challenge = fromHex(input.challenge_hex, 32, "challenge");
  const request = array([
    uint(VERSION), uint(input.old_epoch), bytes(previousTransitionId), bytes(oldPackage.cbor),
    bytes(oldWitness.cbor), bytes(newPackage.cbor), bytes(newWitness.cbor), bytes(challenge),
    uint(input.issued_at_unix_seconds), uint(input.expires_at_unix_seconds),
  ]);
  const requestId = domainHash(DOMAINS.policyRequestId, request);

  function approval(signer, previousIdHex, approvedAt) {
    const content = array([
      uint(VERSION), bytes(root.publicKey), bytes(requestId), uint(input.old_epoch),
      uint(input.old_epoch + 1), bytes(previousTransitionId), bytes(oldRosterDigest),
      bytes(newRosterDigest), bytes(challenge), uint(input.expires_at_unix_seconds),
      bytes(signer.publicKey), bytes(fromHex(previousIdHex, 32, "previous approval ID")),
      uint(approvedAt),
    ]);
    return {
      deviceId: signer.publicKey,
      signed: signed(DOMAINS.policyApprovalSignature, content, signer),
    };
  }
  const approvals = [
    approval(oldDevice, input.old_device_previous_approval_id_hex,
      input.old_device_approved_at_unix_seconds),
    approval(newDevice, input.new_device_previous_approval_id_hex,
      input.new_device_approved_at_unix_seconds),
  ].sort((left, right) => Buffer.compare(left.deviceId, right.deviceId));
  const certificate = array([
    uint(VERSION), bytes(request), array(approvals.map((entry) => bytes(entry.signed.cbor))),
  ]);

  return {
    account_id_hex: root.publicKey.toString("hex"),
    old_device_id_hex: oldDevice.publicKey.toString("hex"),
    new_device_id_hex: newDevice.publicKey.toString("hex"),
    dependencies: {
      old_package_cbor_hex: oldPackage.cbor.toString("hex"),
      old_witness_cbor_hex: oldWitness.cbor.toString("hex"),
      retained_membership_cbor_hex: membership.toString("hex"),
      retained_revocation_cbor_hex: oldRevocation.toString("hex"),
      new_device_certificate: signedJson(newCertificate),
      new_authority_snapshot: signedJson(newSnapshot),
      new_account_device_list: signedJson(newDeviceList),
    },
    new_package: signedJson(newPackage, DOMAINS.packageId),
    new_witness: signedJson(newWitness),
    rosters: {
      old_digest_hex: oldRosterDigest.toString("hex"),
      new_digest_hex: newRosterDigest.toString("hex"),
      old_count: 1,
      new_count: 2,
    },
    request: objectJson(request, DOMAINS.policyRequestId),
    approvals: approvals.map((entry) => ({
      approver_device_id_hex: entry.deviceId.toString("hex"),
      ...signedJson(entry.signed, DOMAINS.policyApprovalId),
    })),
    certificate: objectJson(certificate, DOMAINS.policyCertificateId),
    quorum: {
      old_observed: 1,
      old_required: 1,
      new_observed: 2,
      new_required: 2,
      joint_majority_satisfied: true,
    },
  };
}

function parse(argv) {
  const result = { selfTest: false, emit: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (argument === "--emit") result.emit = true;
    else if (["--schema", "--vectors", "--authority-vectors", "--account-vectors",
      "--recovery-vectors", "--output"].includes(argument)) {
      result[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(result["authority-vectors"], "--authority-vectors is required");
  assert(result["account-vectors"], "--account-vectors is required");
  assert(result["recovery-vectors"], "--recovery-vectors is required");
  if (!result.emit) {
    assert(result.schema, "--schema is required");
    assert(result.vectors, "--vectors is required");
  }
  return result;
}

const options = parse(process.argv.slice(2));
const authority = JSON.parse(readFileSync(options["authority-vectors"], "utf8"));
const account = JSON.parse(readFileSync(options["account-vectors"], "utf8"));
const recovery = JSON.parse(readFileSync(options["recovery-vectors"], "utf8"));
const defaultInput = {
  new_device_secret_key_hex:
    "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
  new_device_encryption_public_key_hex:
    "b0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecf",
  new_certificate_authority_sequence: 9,
  new_authority_revision: 10,
  old_epoch: 0,
  previous_transition_id_hex: "00".repeat(32),
  challenge_hex: "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f",
  issued_at_unix_seconds: 1800001000,
  expires_at_unix_seconds: 1800001600,
  old_device_previous_approval_id_hex:
    "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf",
  new_device_previous_approval_id_hex:
    "c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf",
  old_device_approved_at_unix_seconds: 1800001005,
  new_device_approved_at_unix_seconds: 1800001006,
};

if (options.emit) {
  const document = {
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(authority, account, recovery, defaultInput),
  };
  const output = `${JSON.stringify(document, null, 2)}\n`;
  if (options.output) writeFileSync(options.output, output);
  else process.stdout.write(output);
  process.exit(0);
}

const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.27" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "policy schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "policy vector header changed");
equalJson(generate(authority, account, recovery, vectors.input), vectors.expected,
  "independent Account recovery policy vector");
if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.expected.approvals[0].signature_hex =
    `00${changed.expected.approvals[0].signature_hex.slice(2)}`;
  let rejected = false;
  try {
    equalJson(generate(authority, account, recovery, changed.input), changed.expected,
      "tampered policy vector");
  } catch {
    rejected = true;
  }
  assert(rejected, "tampered transition approval vector was accepted");
  process.stdout.write("canonical_account_recovery_policy_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_account_recovery_policy_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519-and-javascript-blake3\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("joint_old_new_quorum=verified\n");
  process.stdout.write("active_wire_changed=false\n");
}
