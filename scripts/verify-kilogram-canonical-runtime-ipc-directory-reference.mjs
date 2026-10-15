import {
  createPrivateKey,
  createPublicKey,
  sign as signEd25519,
  verify as verifyEd25519,
} from "node:crypto";
import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

const VERSION = 1;
const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const DOMAINS = Object.fromEntries(Object.entries({
  deviceRevocation: "kilogram:device-revocation-signature:cbor-v1\0",
  authoritySnapshot: "kilogram:account-authority-snapshot-signature:cbor-v1\0",
  accountDeviceList: "kilogram:account-device-list-signature:cbor-v1\0",
  deviceListDigest: "kilogram:runtime-ipc-device-list-digest:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-directory-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-directory-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-directory-response-id:cbor-v1\0",
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
const textBytes = (value) => bytes(Buffer.from(value, "utf8"));
const some = (value) => array([uint(1), value]);

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
  const cbor = array([uint(VERSION), content, bytes(signature)]);
  return { content, preimage, signature, cbor };
}
function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((name) => [name, stable(value[name])]));
  }
  return value;
}
function equalJson(actual, expected, label) {
  const actualStable = stable(actual);
  const expectedStable = stable(expected);
  if (JSON.stringify(actualStable) === JSON.stringify(expectedStable)) return;
  const visit = (left, right, path) => {
    if (JSON.stringify(left) === JSON.stringify(right)) return undefined;
    if (left !== null && right !== null && typeof left === "object" && typeof right === "object") {
      const names = [...new Set([...Object.keys(left), ...Object.keys(right)])].sort();
      for (const name of names) {
        const mismatch = visit(left[name], right[name], `${path}/${name}`);
        if (mismatch) return mismatch;
      }
    }
    return `${path}: actual=${JSON.stringify(left)} expected=${JSON.stringify(right)}`;
  };
  fail(`${label} diverged at ${visit(actualStable, expectedStable, "expected")}`);
}
function digestJson(cbor) {
  const digest = domainHash(DOMAINS.deviceListDigest, cbor);
  return {
    signed_cbor_hex: cbor.toString("hex"),
    digest_domain_hex: DOMAINS.deviceListDigest.toString("hex"),
    digest_preimage_hex: Buffer.concat([DOMAINS.deviceListDigest, cbor]).toString("hex"),
    digest_hex: digest.toString("hex"),
  };
}
function requestJson(cbor) {
  const digest = domainHash(DOMAINS.requestDigest, cbor);
  return {
    cbor_hex: cbor.toString("hex"),
    digest_domain_hex: DOMAINS.requestDigest.toString("hex"),
    digest_preimage_hex: Buffer.concat([DOMAINS.requestDigest, cbor]).toString("hex"),
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

function generate(core, recovery, account, authority, input) {
  const device = key(core.input.device_secret_key_hex, "Device seed");
  const root = key(account.input.root_secret_key_hex, "Root seed");
  const accountId = fromHex(core.expected.account_id_hex, 32, "Account ID");
  const localDeviceId = fromHex(core.expected.device_id_hex, 32, "local Device ID");
  const removedDeviceId = fromHex(recovery.expected.new_device_id_hex, 32, "removed Device ID");
  assert(device.publicKey.equals(localDeviceId), "runtime Device dependency changed");
  assert(root.publicKey.equals(accountId), "Account Root dependency changed");
  assert(authority.expected.device_id_hex === core.expected.device_id_hex,
    "authority and runtime Device dependencies diverged");

  const current = fromHex(
    recovery.expected.dependencies.new_account_device_list.signed_cbor_hex,
    undefined,
    "current device list",
  );
  const retainedRevocation = fromHex(
    recovery.expected.dependencies.retained_revocation_cbor_hex,
    undefined,
    "retained revocation",
  );
  const localCertificate = fromHex(
    authority.expected.certificate.signed_cbor_hex,
    undefined,
    "local certificate",
  );
  const previousRevision = recovery.input.new_authority_revision;
  const authorityRevision = previousRevision + 1;

  const removalContent = array([
    uint(VERSION), bytes(accountId), bytes(removedDeviceId), uint(previousRevision),
  ]);
  const removal = signed(DOMAINS.deviceRevocation, removalContent, root);
  const snapshotContent = array([
    uint(VERSION), bytes(accountId), uint(authorityRevision),
    array([retainedRevocation, removal.cbor]),
  ]);
  const snapshot = signed(DOMAINS.authoritySnapshot, snapshotContent, root);
  const listContent = array([
    uint(VERSION), bytes(snapshot.cbor), array([bytes(localCertificate)]),
  ]);
  const candidate = signed(DOMAINS.accountDeviceList, listContent, root);
  const currentDigest = domainHash(DOMAINS.deviceListDigest, current);
  const candidateDigest = domainHash(DOMAINS.deviceListDigest, candidate.cbor);

  const requestId = fromHex(input.request_id_hex, 32, "request ID");
  const command = array([
    uint(0), textBytes(input.device_list_file), uint(previousRevision),
    bytes(currentDigest), bytes(candidate.cbor),
  ]);
  const request = array([
    uint(VERSION),
    bytes(fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID")),
    bytes(requestId), bytes(fromHex(core.input.bearer_token_hex, 32, "bearer token")),
    uint(input.request_issued_at_unix_seconds), uint(input.request_expires_at_unix_seconds),
    command,
  ]);
  const requestDigest = domainHash(DOMAINS.requestDigest, request);

  const status = array([
    uint(VERSION), bytes(accountId), bytes(localDeviceId), textBytes(input.state_directory),
    uint(authorityRevision), uint(1),
    some(bytes(fromHex(input.receipt_id_hex, 32, "receipt ID"))),
    some(uint(input.receipt_generation)), bytes(candidateDigest),
    textBytes(input.device_list_file), textBytes(input.launch_device_list_file),
    uint(1), uint(0),
  ]);
  const applied = array([
    uint(VERSION), bytes(accountId), bytes(localDeviceId), uint(previousRevision),
    uint(authorityRevision), bytes(currentDigest), bytes(candidateDigest), uint(1),
    array([bytes(removedDeviceId)]), uint(input.ratchet_session_records_retired),
    uint(input.prekey_observations_retired), uint(input.pending_unmaterialized_messages),
    uint(input.pending_materialized_messages), uint(input.ticket_published ? 1 : 0),
    uint(input.launch_profile_update_required ? 1 : 0), uint(0), uint(0), uint(0), status,
  ]);
  const result = array([uint(0), applied]);
  const responseContent = array([
    uint(VERSION),
    bytes(fromHex(core.expected.descriptor.object_id_hex, 32, "descriptor ID")),
    bytes(requestId), bytes(requestDigest), bytes(localDeviceId), result,
  ]);
  const response = signed(DOMAINS.responseSignature, responseContent, device);

  return {
    account_id_hex: accountId.toString("hex"),
    active_device_count: 1,
    authority_revision: authorityRevision,
    candidate_device_list: digestJson(candidate.cbor),
    context: {
      active_runtime_changed: false,
      candidate_embedded_in_request: true,
      command_result_pair: "apply-own-device-directory/own-device-directory-applied",
      exact_current_anchor_required: true,
      only_existing_root_revoked_devices_may_disappear: true,
      running_device_must_remain_byte_identical: true,
      valid_at_unix_seconds: input.request_issued_at_unix_seconds,
    },
    current_device_list: digestJson(current),
    local_device_id_hex: localDeviceId.toString("hex"),
    previous_authority_revision: previousRevision,
    removed_device_id_hex: removedDeviceId.toString("hex"),
    request: requestJson(request),
    response: responseJson(response),
  };
}

function parse(argv) {
  const result = { selfTest: false };
  const valued = new Set([
    "--schema", "--vectors", "--core-vectors", "--recovery-vectors",
    "--account-vectors", "--authority-vectors",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") result.selfTest = true;
    else if (valued.has(argument)) { result[argument.slice(2)] = argv[index + 1]; index += 1; }
    else fail(`unknown argument ${argument}`);
  }
  for (const required of valued) assert(result[required.slice(2)], `${required} is required`);
  return result;
}

const options = parse(process.argv.slice(2));
const schema = JSON.parse(readFileSync(options.schema, "utf8"));
const vectors = JSON.parse(readFileSync(options.vectors, "utf8"));
const core = JSON.parse(readFileSync(options["core-vectors"], "utf8"));
const recovery = JSON.parse(readFileSync(options["recovery-vectors"], "utf8"));
const account = JSON.parse(readFileSync(options["account-vectors"], "utf8"));
const authority = JSON.parse(readFileSync(options["authority-vectors"], "utf8"));

assert(schema.milestone === "M1.1.32" && schema.status === "candidate-not-active" &&
  schema.changes_active_wire === false, "runtime IPC schema header changed");
assert(schema.limits.maximum_directory_response_bytes === 131072 &&
  schema.limits.maximum_account_devices === 32 && schema.limits.maximum_path_bytes === 4096,
"runtime IPC directory bounds changed");
assert(schema.activation.current_runtime_encoding === "postcard-v26-with-json-descriptor-v7" &&
  schema.activation.current_runtime_changed === false, "active runtime IPC changed");
assert(vectors.schema_version === 1 && vectors.profile === schema.profile &&
  vectors.activation_status === "not-active", "runtime IPC directory vector header changed");
equalJson(generate(core, recovery, account, authority, vectors.input), vectors.expected,
  "independent canonical runtime IPC directory vector");

if (options.selfTest) {
  const changedPath = structuredClone(vectors.input);
  changedPath.device_list_file = "C:\\Kilogram\\state\\substituted.bin";
  const pathResult = generate(core, recovery, account, authority, changedPath);
  assert(pathResult.request.object_id_hex !== vectors.expected.request.object_id_hex &&
    pathResult.response.signature_hex !== vectors.expected.response.signature_hex,
  "changed device-list path preserved request or response identity");
  const changedPending = structuredClone(vectors.input);
  changedPending.pending_materialized_messages += 1;
  assert(generate(core, recovery, account, authority, changedPending).response.signature_hex !==
    vectors.expected.response.signature_hex,
  "changed mutation result preserved response signature");
  process.stdout.write("changed_directory_path=rejected\n");
  process.stdout.write("changed_directory_result=rejected\n");
  process.stdout.write("canonical_runtime_ipc_directory_reference_self_test=passed\n");
} else {
  process.stdout.write("canonical_runtime_ipc_directory_reference=verified\n");
  process.stdout.write("independent_language=javascript\n");
  process.stdout.write("independent_blake3=verified\n");
  process.stdout.write("root_revoked_removal=verified\n");
  process.stdout.write("active_runtime_ipc_changed=false\n");
}
