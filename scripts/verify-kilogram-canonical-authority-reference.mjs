import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";

const PROFILE_VERSION = 1;
const PKCS8_ED25519_SEED_PREFIX = Buffer.from(
  "302e020100300506032b657004220420",
  "hex",
);
const SPKI_ED25519_PUBLIC_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

const DOMAINS = Object.freeze({
  certificate: Buffer.from(
    "kilogram:device-certificate-signature:cbor-v1\0",
    "utf8",
  ),
  revocation: Buffer.from(
    "kilogram:device-revocation-signature:cbor-v1\0",
    "utf8",
  ),
  authority_snapshot: Buffer.from(
    "kilogram:account-authority-snapshot-signature:cbor-v1\0",
    "utf8",
  ),
  session_authorization: Buffer.from(
    "kilogram:device-session-authorization-signature:cbor-v1\0",
    "utf8",
  ),
});

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function canonicalHex(value, expectedBytes, label) {
  assert(
    typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} is not non-empty lowercase hexadecimal`,
  );
  const bytes = Buffer.from(value, "hex");
  assert(bytes.toString("hex") === value, `${label} does not round-trip as hexadecimal`);
  if (expectedBytes !== undefined) {
    assert(bytes.length === expectedBytes, `${label} must contain ${expectedBytes} bytes`);
  }
  return bytes;
}

function uintHead(majorType, value) {
  assert(Number.isSafeInteger(value) && value >= 0, "CBOR unsigned value is out of range");
  const major = majorType << 5;
  if (value <= 23) return Buffer.from([major | value]);
  if (value <= 0xff) return Buffer.from([major | 24, value]);
  if (value <= 0xffff) {
    const result = Buffer.alloc(3);
    result[0] = major | 25;
    result.writeUInt16BE(value, 1);
    return result;
  }
  if (value <= 0xffffffff) {
    const result = Buffer.alloc(5);
    result[0] = major | 26;
    result.writeUInt32BE(value, 1);
    return result;
  }
  const result = Buffer.alloc(9);
  result[0] = major | 27;
  result.writeBigUInt64BE(BigInt(value), 1);
  return result;
}

function cborUnsigned(value) {
  return uintHead(0, value);
}

function cborBytes(value) {
  return Buffer.concat([uintHead(2, value.length), value]);
}

function cborArray(values) {
  return Buffer.concat([uintHead(4, values.length), ...values]);
}

function privateKeyFromSeed(seed) {
  return createPrivateKey({
    key: Buffer.concat([PKCS8_ED25519_SEED_PREFIX, seed]),
    format: "der",
    type: "pkcs8",
  });
}

function publicKeyBytes(privateKey) {
  const spki = createPublicKey(privateKey).export({ format: "der", type: "spki" });
  assert(
    spki.subarray(0, SPKI_ED25519_PUBLIC_PREFIX.length).equals(SPKI_ED25519_PUBLIC_PREFIX),
    "Node returned an unexpected Ed25519 SPKI prefix",
  );
  assert(spki.length === SPKI_ED25519_PUBLIC_PREFIX.length + 32, "unexpected Ed25519 key size");
  return spki.subarray(SPKI_ED25519_PUBLIC_PREFIX.length);
}

function publicKeyFromRaw(raw) {
  return createPublicKey({
    key: Buffer.concat([SPKI_ED25519_PUBLIC_PREFIX, raw]),
    format: "der",
    type: "spki",
  });
}

function encodeCertificate(input, expected) {
  assert(
    Array.isArray(input.capabilities) &&
      input.capabilities.length >= 1 &&
      input.capabilities.length <= 2,
    "capabilities must contain one or two entries",
  );
  for (let index = 0; index < input.capabilities.length; index += 1) {
    const capability = input.capabilities[index];
    assert(capability === 1 || capability === 2, "unknown capability code");
    if (index > 0) {
      assert(input.capabilities[index - 1] < capability, "capabilities are not strictly ordered");
    }
  }
  return cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(canonicalHex(expected.account_id_hex, 32, "Account ID")),
    cborBytes(canonicalHex(expected.device_id_hex, 32, "Device ID")),
    cborBytes(canonicalHex(input.encryption_public_key_hex, 32, "encryption public key")),
    cborUnsigned(input.certificate_authority_sequence),
    cborArray(input.capabilities.map(cborUnsigned)),
  ]);
}

function encodeRevocation(input, expected) {
  return cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(canonicalHex(expected.account_id_hex, 32, "revocation Account ID")),
    cborBytes(canonicalHex(expected.revoked_device_id_hex, 32, "revoked Device ID")),
    cborUnsigned(input.revocation_authority_sequence),
  ]);
}

function signedEnvelope(content, signature) {
  return cborArray([
    cborUnsigned(PROFILE_VERSION),
    content,
    cborBytes(signature),
  ]);
}

function assertObjectVector(name, expected, domain, content, privateKey, publicKey) {
  const expectedContent = canonicalHex(expected.content_cbor_hex, undefined, `${name} content`);
  assert(content.equals(expectedContent), `${name} content bytes diverged`);
  assert(
    domain.equals(canonicalHex(expected.signature_domain_hex, undefined, `${name} domain`)),
    `${name} domain diverged`,
  );
  const preimage = Buffer.concat([domain, content]);
  assert(
    preimage.equals(canonicalHex(expected.signing_preimage_hex, undefined, `${name} preimage`)),
    `${name} signing preimage diverged`,
  );
  const signature = canonicalHex(expected.signature_hex, 64, `${name} signature`);
  const independentlySigned = signEd25519(null, preimage, privateKey);
  assert(independentlySigned.equals(signature), `${name} deterministic signature diverged`);
  assert(
    verifyEd25519(null, preimage, publicKey, signature),
    `${name} signature failed independent verification`,
  );
  const signed = signedEnvelope(content, signature);
  assert(
    signed.equals(canonicalHex(expected.signed_cbor_hex, undefined, `${name} signed envelope`)),
    `${name} signed envelope diverged`,
  );
  return signed;
}

function verifyContract(schema, vectors) {
  assert(schema.schema_version === 1, "authority schema version changed");
  assert(schema.milestone === "M1.1.2", "authority milestone changed");
  assert(schema.status === "candidate-not-active", "authority schema claims activation");
  assert(schema.changes_active_wire === false, "authority schema claims an active wire change");
  assert(schema.profile === "kilogram-deterministic-cbor-v1", "authority profile changed");
  assert(
    schema.activation?.current_runtime_encoding === "postcard" &&
      schema.activation?.current_runtime_changed === false,
    "active Postcard boundary changed",
  );
  assert(vectors.schema_version === 1, "authority vector version changed");
  assert(vectors.profile === schema.profile, "authority vector profile changed");
  assert(vectors.activation_status === "not-active", "authority vectors claim activation");

  const input = vectors.input;
  const expected = vectors.expected;
  const rootSeed = canonicalHex(input.root_secret_key_hex, 32, "Root test seed");
  const deviceSeed = canonicalHex(input.device_secret_key_hex, 32, "Device test seed");
  const revokedSeed = canonicalHex(
    input.revoked_device_secret_key_hex,
    32,
    "revoked Device test seed",
  );
  const rootPrivate = privateKeyFromSeed(rootSeed);
  const devicePrivate = privateKeyFromSeed(deviceSeed);
  const revokedPrivate = privateKeyFromSeed(revokedSeed);
  const accountId = canonicalHex(expected.account_id_hex, 32, "expected Account ID");
  const deviceId = canonicalHex(expected.device_id_hex, 32, "expected Device ID");
  const revokedDeviceId = canonicalHex(
    expected.revoked_device_id_hex,
    32,
    "expected revoked Device ID",
  );
  assert(publicKeyBytes(rootPrivate).equals(accountId), "Root seed does not derive Account ID");
  assert(publicKeyBytes(devicePrivate).equals(deviceId), "Device seed does not derive Device ID");
  assert(
    publicKeyBytes(revokedPrivate).equals(revokedDeviceId),
    "revoked Device seed does not derive Device ID",
  );
  const rootPublic = publicKeyFromRaw(accountId);
  const devicePublic = publicKeyFromRaw(deviceId);

  const certificateContent = encodeCertificate(input, expected);
  const signedCertificate = assertObjectVector(
    "certificate",
    expected.certificate,
    DOMAINS.certificate,
    certificateContent,
    rootPrivate,
    rootPublic,
  );

  assert(
    input.revocation_authority_sequence < input.authority_revision,
    "revocation is not before authority revision",
  );
  const revocationContent = encodeRevocation(input, expected);
  const signedRevocation = assertObjectVector(
    "revocation",
    expected.revocation,
    DOMAINS.revocation,
    revocationContent,
    rootPrivate,
    rootPublic,
  );

  const snapshotContent = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(accountId),
    cborUnsigned(input.authority_revision),
    cborArray([signedRevocation]),
  ]);
  const signedSnapshot = assertObjectVector(
    "authority snapshot",
    expected.authority_snapshot,
    DOMAINS.authority_snapshot,
    snapshotContent,
    rootPrivate,
    rootPublic,
  );

  assert(
    input.certificate_authority_sequence < input.authority_revision,
    "certificate is not before authority revision",
  );
  assert(!deviceId.equals(revokedDeviceId), "session Device is revoked by the fixture snapshot");
  const sessionContent = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(canonicalHex(input.session_binding_hex, 32, "session binding")),
    signedCertificate,
    signedSnapshot,
  ]);
  assertObjectVector(
    "session authorization",
    expected.session_authorization,
    DOMAINS.session_authorization,
    sessionContent,
    devicePrivate,
    devicePublic,
  );
}

function deepClone(value) {
  return JSON.parse(JSON.stringify(value));
}

function assertRejected(action, label) {
  try {
    action();
  } catch {
    process.stdout.write(`${label}=rejected\n`);
    return;
  }
  fail(`${label} was unexpectedly accepted`);
}

function parseArguments(argv) {
  const parsed = { schema: undefined, vectors: undefined, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      parsed.selfTest = true;
    } else if (argument === "--schema" || argument === "--vectors") {
      const value = argv[index + 1];
      assert(value !== undefined, `${argument} requires a path`);
      parsed[argument.slice(2)] = value;
      index += 1;
    } else {
      fail(`unknown argument ${argument}`);
    }
  }
  assert(parsed.schema !== undefined, "--schema is required");
  assert(parsed.vectors !== undefined, "--vectors is required");
  return parsed;
}

const options = parseArguments(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
verifyContract(schema, vectors);

if (options.selfTest) {
  const active = deepClone(schema);
  active.status = "active";
  assertRejected(() => verifyContract(active, vectors), "active_without_migration");

  const tampered = deepClone(vectors);
  tampered.expected.session_authorization.signature_hex =
    `00${tampered.expected.session_authorization.signature_hex.slice(2)}`;
  assertRejected(() => verifyContract(schema, tampered), "tampered_signature");

  const uppercase = deepClone(vectors);
  uppercase.input.session_binding_hex = uppercase.input.session_binding_hex.toUpperCase();
  assertRejected(() => verifyContract(schema, uppercase), "noncanonical_hex");

  const unknownCapability = deepClone(vectors);
  unknownCapability.input.capabilities = [1, 3];
  assertRejected(() => verifyContract(schema, unknownCapability), "unknown_capability");

  process.stdout.write("canonical_authority_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_authority_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519\n");
  process.stdout.write("active_wire_changed=false\n");
}
