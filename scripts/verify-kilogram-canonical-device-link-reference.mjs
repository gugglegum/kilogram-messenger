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
const DOMAIN = Buffer.from(
  "kilogram:device-link-authorization-signature:cbor-v1\0",
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
function generate(authority, account, input) {
  const rootPrivate = privateFromSeed(fromHex(input.root_secret_key_hex, 32, "Root seed"));
  const accountId = publicBytes(rootPrivate);
  assert(accountId.toString("hex") === authority.expected.account_id_hex,
    "account Root does not match authority fixture");
  assert(accountId.toString("hex") === account.expected.account_id_hex,
    "account Root does not match account fixture");
  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const certificate = fromHex(
    authority.expected.certificate.signed_cbor_hex, undefined, "device certificate",
  );
  const deviceList = fromHex(
    account.expected.account_device_list.signed_cbor_hex, undefined, "device list",
  );
  const content = array([
    uint(VERSION), bytes(accountId), bytes(requestId), bytes(certificate), bytes(deviceList),
  ]);
  const preimage = Buffer.concat([DOMAIN, content]);
  const signature = signEd25519(null, preimage, rootPrivate);
  const envelope = array([uint(VERSION), content, bytes(signature)]);
  return {
    account_id_hex: accountId.toString("hex"),
    device_id_hex: authority.expected.device_id_hex,
    authorization: {
      content_cbor_hex: content.toString("hex"),
      signature_domain_hex: DOMAIN.toString("hex"),
      signing_preimage_hex: preimage.toString("hex"),
      signature_hex: signature.toString("hex"),
      signed_cbor_hex: envelope.toString("hex"),
    },
  };
}
function equalJson(actual, expected, label) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label} diverged`);
}
function parse(argv) {
  const value = { emit: false, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--emit") value.emit = true;
    else if (argument === "--self-test") value.selfTest = true;
    else if (["--schema", "--vectors", "--authority-vectors", "--account-vectors"].includes(argument)) {
      value[argument.slice(2)] = argv[index + 1]; index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(value["authority-vectors"], "--authority-vectors is required");
  assert(value["account-vectors"], "--account-vectors is required");
  if (!value.emit) { assert(value.schema, "--schema is required"); assert(value.vectors, "--vectors is required"); }
  return value;
}

const options = parse(process.argv.slice(2));
const authority = JSON.parse(readFileSync(options["authority-vectors"], "utf8"));
const account = JSON.parse(readFileSync(options["account-vectors"], "utf8"));
const defaultInput = {
  root_secret_key_hex: authority.input.root_secret_key_hex,
  request_id_hex: "e0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff",
};
if (options.emit) {
  process.stdout.write(`${JSON.stringify({
    schema_version: 1,
    profile: "kilogram-deterministic-cbor-v1",
    activation_status: "not-active",
    input: defaultInput,
    expected: generate(authority, account, defaultInput),
  }, null, 2)}\n`);
  process.exit(0);
}
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
assert(schema.milestone === "M1.1.9" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "device-link schema header changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "device-link vector header changed");
const generated = generate(authority, account, vectors.input);
equalJson(generated, vectors.expected, "independent device-link vector");
const accountId = fromHex(generated.account_id_hex, 32, "Account ID");
const authorization = vectors.expected.authorization;
const preimage = fromHex(authorization.signing_preimage_hex, undefined, "authorization preimage");
assert(preimage.equals(Buffer.concat([
  DOMAIN, fromHex(authorization.content_cbor_hex, undefined, "authorization content"),
])), "authorization preimage diverged");
assert(verifyEd25519(null, preimage, publicFromBytes(accountId),
  fromHex(authorization.signature_hex, 64, "authorization signature")),
"authorization signature failed");
if (options.selfTest) {
  const changed = JSON.parse(JSON.stringify(vectors));
  changed.input.request_id_hex = `00${changed.input.request_id_hex.slice(2)}`;
  let rejected = false;
  try { equalJson(generate(authority, account, changed.input), changed.expected, "changed request"); } catch { rejected = true; }
  assert(rejected, "changed request was accepted");
  process.stdout.write("canonical_device_link_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_device_link_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519\n");
  process.stdout.write("active_wire_changed=false\n");
}
