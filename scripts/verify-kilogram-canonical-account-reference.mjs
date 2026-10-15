import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DEVICE_LIST_DOMAIN = Buffer.from(
  "kilogram:account-device-list-signature:cbor-v1\0",
  "utf8",
);
const MEMBERSHIP_DOMAIN = Buffer.from(
  "kilogram:conversation-membership-signature:cbor-v1\0",
  "utf8",
);

function fail(message) { throw new Error(message); }
function assert(condition, message) { if (!condition) fail(message); }
function head(major, value) {
  assert(Number.isSafeInteger(value) && value >= 0, "invalid CBOR argument");
  const prefix = major << 5;
  if (value <= 23) return Buffer.from([prefix | value]);
  if (value <= 0xff) return Buffer.from([prefix | 24, value]);
  if (value <= 0xffff) {
    const result = Buffer.alloc(3); result[0] = prefix | 25; result.writeUInt16BE(value, 1); return result;
  }
  if (value <= 0xffffffff) {
    const result = Buffer.alloc(5); result[0] = prefix | 26; result.writeUInt32BE(value, 1); return result;
  }
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
  return { content, preimage, signature, envelope: array([uint(VERSION), content, bytes(signature)]) };
}
function objectExpected(domain, value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signature_domain_hex: domain.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.envelope.toString("hex"),
  };
}
function generate(authority, input) {
  const rootSeed = fromHex(input.root_secret_key_hex, 32, "Root seed");
  const otherSeed = fromHex(input.other_member_secret_key_hex, 32, "other member seed");
  const rootPrivate = privateFromSeed(rootSeed);
  const accountId = publicBytes(rootPrivate);
  assert(accountId.toString("hex") === authority.expected.account_id_hex,
    "account Root does not match authority fixture");
  const otherAccountId = publicBytes(privateFromSeed(otherSeed));
  const snapshot = fromHex(authority.expected.authority_snapshot.signed_cbor_hex, undefined, "snapshot");
  const certificate = fromHex(authority.expected.certificate.signed_cbor_hex, undefined, "certificate");
  const deviceListContent = array([
    uint(VERSION), bytes(snapshot), array([bytes(certificate)]),
  ]);
  const deviceList = signed(DEVICE_LIST_DOMAIN, deviceListContent, rootPrivate);
  const members = [accountId, otherAccountId].sort(Buffer.compare);
  const membershipContent = array([
    uint(VERSION),
    bytes(fromHex(input.conversation_id_hex, 32, "conversation ID")),
    uint(input.membership_revision),
    bytes(accountId),
    array(members.map(bytes)),
  ]);
  const membership = signed(MEMBERSHIP_DOMAIN, membershipContent, rootPrivate);
  return {
    account_id_hex: accountId.toString("hex"),
    other_member_account_id_hex: otherAccountId.toString("hex"),
    ordered_members_hex: members.map((member) => member.toString("hex")),
    account_device_list: objectExpected(DEVICE_LIST_DOMAIN, deviceList),
    conversation_membership: objectExpected(MEMBERSHIP_DOMAIN, membership),
  };
}
function verifyObject(name, expected, domain, accountId) {
  const content = fromHex(expected.content_cbor_hex, undefined, `${name} content`);
  const preimage = Buffer.concat([domain, content]);
  assert(preimage.toString("hex") === expected.signing_preimage_hex, `${name} preimage diverged`);
  assert(domain.toString("hex") === expected.signature_domain_hex, `${name} domain diverged`);
  const signature = fromHex(expected.signature_hex, 64, `${name} signature`);
  assert(verifyEd25519(null, preimage, publicFromBytes(accountId), signature), `${name} signature failed`);
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  assert(envelope.toString("hex") === expected.signed_cbor_hex, `${name} envelope diverged`);
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function parse(argv) {
  const value = { selfTest: false, emit: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") value.selfTest = true;
    else if (argument === "--emit") value.emit = true;
    else if (["--schema", "--vectors", "--authority-vectors"].includes(argument)) {
      value[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(value["authority-vectors"], "--authority-vectors is required");
  if (!value.emit) { assert(value.schema, "--schema is required"); assert(value.vectors, "--vectors is required"); }
  return value;
}

const options = parse(process.argv.slice(2));
const authority = JSON.parse(readFileSync(options["authority-vectors"], "utf8"));
const defaultInput = {
  root_secret_key_hex: authority.input.root_secret_key_hex,
  other_member_secret_key_hex: "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
  conversation_id_hex: "d0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeef",
  membership_revision: 3,
};
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(authority, defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.8" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "account schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "account vector header changed");
const generated = generate(authority, vectors.input);
equalJson(generated, vectors.expected, "independent account vector");
const accountId = fromHex(generated.account_id_hex, 32, "Account ID");
verifyObject("account device list", vectors.expected.account_device_list, DEVICE_LIST_DOMAIN, accountId);
verifyObject("conversation membership", vectors.expected.conversation_membership, MEMBERSHIP_DOMAIN, accountId);
if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.expected.conversation_membership.signature_hex =
    `00${changed.expected.conversation_membership.signature_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(authority, changed.input), changed.expected, "tampered vector"); } catch { rejected = true; }
  assert(rejected, "tampered vector was accepted");
  process.stdout.write("canonical_account_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_account_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519\n");
  process.stdout.write("active_wire_changed=false\n");
}
