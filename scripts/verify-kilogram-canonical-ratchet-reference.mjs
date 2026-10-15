import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";

const PROFILE_VERSION = 1;
const PKCS8_ED25519_SEED_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_ED25519_PUBLIC_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.freeze({
  certificate: Buffer.from("kilogram:device-certificate-signature:cbor-v1\0", "utf8"),
  snapshot: Buffer.from(
    "kilogram:account-authority-snapshot-signature:cbor-v1\0",
    "utf8",
  ),
  identity: Buffer.from("kilogram:ratchet-identity-signature:cbor-v1\0", "utf8"),
  bundle: Buffer.from("kilogram:prekey-bundle-signature:cbor-v1\0", "utf8"),
  pool: Buffer.from("kilogram:prekey-pool-signature:cbor-v1\0", "utf8"),
  poolId: Buffer.from("kilogram:prekey-pool-id:cbor-v1\0", "utf8"),
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

const cborUnsigned = (value) => uintHead(0, value);
const cborBytes = (value) => Buffer.concat([uintHead(2, value.length), value]);
const cborArray = (values) => Buffer.concat([uintHead(4, values.length), ...values]);

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
  return spki.subarray(SPKI_ED25519_PUBLIC_PREFIX.length);
}

function publicKeyFromRaw(raw) {
  return createPublicKey({
    key: Buffer.concat([SPKI_ED25519_PUBLIC_PREFIX, raw]),
    format: "der",
    type: "spki",
  });
}

function signedEnvelope(content, signature) {
  return cborArray([cborUnsigned(PROFILE_VERSION), content, cborBytes(signature)]);
}

function assertSigned(name, expected, domain, content, privateKey, publicKey) {
  assert(
    content.equals(canonicalHex(expected.content_cbor_hex, undefined, `${name} content`)),
    `${name} content diverged`,
  );
  assert(
    domain.equals(canonicalHex(expected.signature_domain_hex, undefined, `${name} domain`)),
    `${name} domain diverged`,
  );
  const preimage = Buffer.concat([domain, content]);
  assert(
    preimage.equals(canonicalHex(expected.signing_preimage_hex, undefined, `${name} preimage`)),
    `${name} preimage diverged`,
  );
  const signature = canonicalHex(expected.signature_hex, 64, `${name} signature`);
  assert(signEd25519(null, preimage, privateKey).equals(signature), `${name} signature diverged`);
  assert(verifyEd25519(null, preimage, publicKey, signature), `${name} signature is invalid`);
  const signed = signedEnvelope(content, signature);
  assert(
    signed.equals(canonicalHex(expected.signed_cbor_hex, undefined, `${name} envelope`)),
    `${name} envelope diverged`,
  );
  return signed;
}

function verifyContract(schema, vectors) {
  assert(schema.schema_version === 1 && schema.milestone === "M1.1.3", "schema header changed");
  assert(schema.status === "candidate-not-active", "schema claims activation");
  assert(schema.changes_active_wire === false, "schema claims an active wire change");
  assert(schema.profile === "kilogram-deterministic-cbor-v1", "profile changed");
  assert(
    schema.activation?.current_runtime_encoding === "postcard" &&
      schema.activation?.current_runtime_changed === false,
    "active Postcard boundary changed",
  );
  assert(vectors.schema_version === 1, "vector version changed");
  assert(vectors.profile === schema.profile, "vector profile changed");
  assert(vectors.activation_status === "not-active", "vectors claim activation");

  const input = vectors.input;
  const expected = vectors.expected;
  const rootPrivate = privateKeyFromSeed(canonicalHex(input.root_secret_key_hex, 32, "Root seed"));
  const devicePrivate = privateKeyFromSeed(
    canonicalHex(input.device_secret_key_hex, 32, "Device seed"),
  );
  const accountId = publicKeyBytes(rootPrivate);
  const deviceId = publicKeyBytes(devicePrivate);
  assert(accountId.equals(canonicalHex(expected.account_id_hex, 32, "Account ID")), "Account ID diverged");
  assert(deviceId.equals(canonicalHex(expected.device_id_hex, 32, "Device ID")), "Device ID diverged");
  const rootPublic = publicKeyFromRaw(accountId);
  const devicePublic = publicKeyFromRaw(deviceId);

  const certificateContent = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(accountId),
    cborBytes(deviceId),
    cborBytes(canonicalHex(input.encryption_public_key_hex, 32, "encryption key")),
    cborUnsigned(input.certificate_authority_sequence),
    cborArray([cborUnsigned(1), cborUnsigned(2)]),
  ]);
  const certificate = assertSigned(
    "certificate",
    expected.certificate,
    DOMAINS.certificate,
    certificateContent,
    rootPrivate,
    rootPublic,
  );

  const snapshotContent = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(accountId),
    cborUnsigned(input.authority_revision),
    cborArray([]),
  ]);
  const snapshot = assertSigned(
    "authority snapshot",
    expected.authority_snapshot,
    DOMAINS.snapshot,
    snapshotContent,
    rootPrivate,
    rootPublic,
  );

  const identityContent = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(deviceId),
    cborBytes(canonicalHex(input.curve25519_key_hex, 32, "Curve25519 key")),
    cborBytes(canonicalHex(input.olm_ed25519_key_hex, 32, "Olm Ed25519 key")),
  ]);
  const identity = assertSigned(
    "ratchet identity",
    expected.ratchet_identity,
    DOMAINS.identity,
    identityContent,
    devicePrivate,
    devicePublic,
  );

  assert(input.prekey_sequences.length === input.one_time_key_hexes.length, "prekey input lengths differ");
  assert(expected.prekey_bundles.length === input.prekey_sequences.length, "prekey vector lengths differ");
  const bundles = [];
  for (let index = 0; index < input.prekey_sequences.length; index += 1) {
    if (index > 0) {
      assert(
        input.prekey_sequences[index] === input.prekey_sequences[index - 1] + 1,
        "prekey sequences are not contiguous",
      );
    }
    const content = cborArray([
      cborUnsigned(PROFILE_VERSION),
      cborBytes(identity),
      cborUnsigned(input.prekey_sequences[index]),
      cborBytes(canonicalHex(input.one_time_key_hexes[index], 32, `one-time key ${index}`)),
    ]);
    bundles.push(
      assertSigned(
        `prekey bundle ${index}`,
        expected.prekey_bundles[index],
        DOMAINS.bundle,
        content,
        devicePrivate,
        devicePublic,
      ),
    );
  }
  assert(input.published_at_unix_seconds < input.expires_at_unix_seconds, "invalid validity interval");
  assert(
    input.expires_at_unix_seconds - input.published_at_unix_seconds <= 2592000,
    "validity interval exceeds 30 days",
  );
  const poolContent = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(identity),
    cborUnsigned(input.pool_generation),
    cborUnsigned(input.published_at_unix_seconds),
    cborUnsigned(input.expires_at_unix_seconds),
    cborArray(bundles.map(cborBytes)),
  ]);
  const pool = assertSigned(
    "prekey pool",
    expected.prekey_pool,
    DOMAINS.pool,
    poolContent,
    devicePrivate,
    devicePublic,
  );

  assert(
    DOMAINS.poolId.equals(
      canonicalHex(expected.prekey_pool_id_domain_hex, undefined, "prekey pool ID domain"),
    ),
    "prekey pool ID domain diverged",
  );
  canonicalHex(expected.prekey_pool_id_hex, 32, "prekey pool ID");
  const directory = cborArray([
    cborUnsigned(PROFILE_VERSION),
    cborBytes(snapshot),
    cborArray([cborBytes(certificate)]),
    cborArray([cborBytes(pool)]),
  ]);
  assert(
    directory.equals(
      canonicalHex(expected.account_prekey_directory_cbor_hex, undefined, "prekey directory"),
    ),
    "prekey directory diverged",
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
    if (argument === "--self-test") parsed.selfTest = true;
    else if (argument === "--schema" || argument === "--vectors") {
      parsed[argument.slice(2)] = argv[index + 1];
      assert(parsed[argument.slice(2)] !== undefined, `${argument} requires a path`);
      index += 1;
    } else fail(`unknown argument ${argument}`);
  }
  assert(parsed.schema !== undefined && parsed.vectors !== undefined, "schema and vectors are required");
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
  tampered.expected.prekey_pool.signature_hex =
    `00${tampered.expected.prekey_pool.signature_hex.slice(2)}`;
  assertRejected(() => verifyContract(schema, tampered), "tampered_pool_signature");

  const gap = deepClone(vectors);
  gap.input.prekey_sequences[1] += 1;
  assertRejected(() => verifyContract(schema, gap), "non_contiguous_prekeys");

  const uppercase = deepClone(vectors);
  uppercase.input.curve25519_key_hex = uppercase.input.curve25519_key_hex.toUpperCase();
  assertRejected(() => verifyContract(schema, uppercase), "noncanonical_hex");
  process.stdout.write("canonical_ratchet_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_ratchet_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_crypto=node-crypto-ed25519\n");
  process.stdout.write("independent_blake3=pending\n");
  process.stdout.write("active_wire_changed=false\n");
}
