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
  planSignature: "kilogram:history-recovery-plan-signature:cbor-v1\0",
  planId: "kilogram:history-recovery-plan-id:cbor-v1\0",
  cancellationSignature:
    "kilogram:runtime-ipc-recovery-plan-cancellation-signature:cbor-v1\0",
  recordId: "kilogram:runtime-ipc-recovery-plan-record-id:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-recovery-plan-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-recovery-plan-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-recovery-plan-response-id:cbor-v1\0",
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
const none = () => array([uint(0)]);
const someBytes = (value) => array([uint(1), bytes(value)]);

function fromHex(value, length, label) {
  assert(typeof value === "string" && /^[0-9a-f]+$/.test(value) && value.length % 2 === 0,
    `${label} must be lowercase even-length hexadecimal`);
  const output = Buffer.from(value, "hex");
  if (length !== undefined) assert(output.length === length, `${label} must contain ${length} bytes`);
  return output;
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
function sign(domain, content, signer) {
  const preimage = Buffer.concat([domain, content]);
  const signature = signEd25519(null, preimage, signer);
  assert(verifyEd25519(null, preimage, publicFromBytes(publicBytes(signer)), signature),
    "generated Ed25519 signature failed");
  return { content, preimage, signature, cbor: array([uint(VERSION), content, bytes(signature)]) };
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

function executionPolicy(input) {
  return array([
    uint(VERSION), uint(input.allow_ethernet ? 1 : 0), uint(input.allow_wifi ? 1 : 0),
    uint(input.allow_mobile ? 1 : 0), uint(input.allow_unknown_network ? 1 : 0),
    uint(input.require_external_power ? 1 : 0),
  ]);
}
function state(recordId, lifecycle, signedPlan, cancellation) {
  return array([
    uint(VERSION), bytes(recordId), uint(lifecycle), bytes(signedPlan),
    cancellation === undefined ? none() : someBytes(cancellation),
  ]);
}
function custody(stateCbor) {
  return array([uint(VERSION), stateCbor, uint(0), uint(0), uint(0)]);
}
function recordId(signedPlan, cancellation) {
  return domainHash(DOMAINS.recordId, array([
    uint(VERSION), bytes(signedPlan), cancellation === undefined ? none() : someBytes(cancellation),
  ]));
}
function request(descriptorId, requestId, bearer, issuedAt, expiresAt, command) {
  const cbor = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(issuedAt), uint(expiresAt), command,
  ]);
  return { cbor, digest: domainHash(DOMAINS.requestDigest, cbor) };
}
function response(descriptorId, requestId, requestDigest, device, result, signer) {
  const content = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(requestDigest), bytes(device), result,
  ]);
  return sign(DOMAINS.responseSignature, content, signer);
}
function caseJson(requestValue, responseValue) {
  return {
    request: {
      cbor_hex: requestValue.cbor.toString("hex"),
      digest_hex: requestValue.digest.toString("hex"),
    },
    response: {
      content_cbor_hex: responseValue.content.toString("hex"),
      signing_preimage_hex: responseValue.preimage.toString("hex"),
      signature_hex: responseValue.signature.toString("hex"),
      signed_cbor_hex: responseValue.cbor.toString("hex"),
      id_hex: domainHash(DOMAINS.responseId, responseValue.cbor).toString("hex"),
    },
  };
}

function generate(input) {
  const signer = privateKey(fromHex(input.device_secret_key_hex, 32, "Device secret"));
  const device = publicBytes(signer);
  assert(device.equals(fromHex(input.recipient_device_id_hex, 32, "recipient Device ID")),
    "Device secret does not match recipient Device ID");
  const account = fromHex(input.account_id_hex, 32, "Account ID");
  const descriptorId = fromHex(input.descriptor_id_hex, 32, "descriptor ID");
  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const link = fromHex(input.signed_link_cbor_hex, undefined, "signed recovery link");
  const sas = fromHex(input.confirmed_sas_hex, 32, "confirmed SAS");
  const policy = executionPolicy(input);

  const planContent = array([
    uint(VERSION), bytes(link), bytes(sas), policy,
    uint(input.approve_issued_at_unix_seconds),
    uint(input.approve_issued_at_unix_seconds + input.valid_for_seconds),
  ]);
  const plan = sign(DOMAINS.planSignature, planContent, signer);
  const planId = domainHash(DOMAINS.planId, plan.cbor);
  const approvedRecordId = recordId(plan.cbor, undefined);
  const approvedState = state(approvedRecordId, 0, plan.cbor, undefined);
  const approvedCustody = custody(approvedState);

  const cancellationContent = array([
    uint(VERSION), bytes(planId), bytes(account), bytes(device),
    uint(input.cancel_issued_at_unix_seconds), uint(0),
  ]);
  const cancellation = sign(DOMAINS.cancellationSignature, cancellationContent, signer);
  const cancelledRecordId = recordId(plan.cbor, cancellation.cbor);
  const cancelledState = state(cancelledRecordId, 1, plan.cbor, cancellation.cbor);
  const cancelledCustody = custody(cancelledState);

  const approveRequest = request(
    descriptorId, fromHex(input.approve_request_id_hex, 32, "approve request ID"), bearer,
    input.approve_issued_at_unix_seconds, input.approve_expires_at_unix_seconds,
    array([uint(0), bytes(link), bytes(sas), policy, uint(input.valid_for_seconds)]),
  );
  const statusRequest = request(
    descriptorId, fromHex(input.status_request_id_hex, 32, "status request ID"), bearer,
    input.status_issued_at_unix_seconds, input.status_expires_at_unix_seconds,
    array([uint(1), bytes(planId)]),
  );
  const cancelRequest = request(
    descriptorId, fromHex(input.cancel_request_id_hex, 32, "cancel request ID"), bearer,
    input.cancel_issued_at_unix_seconds, input.cancel_expires_at_unix_seconds,
    array([uint(2), bytes(planId), bytes(approvedRecordId)]),
  );
  const approveResponse = response(
    descriptorId, fromHex(input.approve_request_id_hex, 32, "approve request ID"),
    approveRequest.digest, device, array([uint(0), uint(0), approvedCustody]), signer,
  );
  const statusResponse = response(
    descriptorId, fromHex(input.status_request_id_hex, 32, "status request ID"),
    statusRequest.digest, device, array([uint(1), approvedCustody]), signer,
  );
  const cancelResponse = response(
    descriptorId, fromHex(input.cancel_request_id_hex, 32, "cancel request ID"),
    cancelRequest.digest, device, array([uint(2), uint(0), cancelledCustody]), signer,
  );

  return {
    plan: {
      signed_cbor_hex: plan.cbor.toString("hex"),
      plan_id_hex: planId.toString("hex"),
      approved_record_id_hex: approvedRecordId.toString("hex"),
    },
    cancellation: {
      content_cbor_hex: cancellation.content.toString("hex"),
      signing_preimage_hex: cancellation.preimage.toString("hex"),
      signature_hex: cancellation.signature.toString("hex"),
      signed_cbor_hex: cancellation.cbor.toString("hex"),
      cancelled_record_id_hex: cancelledRecordId.toString("hex"),
    },
    approve: caseJson(approveRequest, approveResponse),
    status: caseJson(statusRequest, statusResponse),
    cancel: caseJson(cancelRequest, cancelResponse),
  };
}

function argumentsMap(argv) {
  const output = new Map();
  for (let index = 2; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === "--self-test") { output.set(key, true); continue; }
    const value = argv[index + 1]; assert(value !== undefined, `missing value after ${key}`);
    output.set(key, value); index += 1;
  }
  return output;
}

const args = argumentsMap(process.argv);
for (const required of ["--schema", "--vectors"]) assert(args.has(required), `missing ${required}`);
const schema = JSON.parse(readFileSync(args.get("--schema"), "utf8"));
const vectors = JSON.parse(readFileSync(args.get("--vectors"), "utf8"));
assert(schema.milestone === "M1.1.49" && schema.status === "candidate-not-active",
  "runtime IPC schema is not the expected inactive M1.1.49 candidate");
assert(schema.domains.length === 78 && schema.objects.length === 38 &&
  schema.covered_commands.length === 39 && schema.covered_results.length === 38,
  "runtime IPC schema registry counts changed");
assert(vectors.schema_version === 1 && vectors.milestone === "M1.1.43" &&
  vectors.status === "candidate-not-active" && vectors.changes_active_runtime_ipc === false,
  "recovery-plan vector header changed");
for (const [name, domain] of Object.entries({
  cancellation_signature: DOMAINS.cancellationSignature,
  record_id: DOMAINS.recordId,
  request_digest: DOMAINS.requestDigest,
  response_signature: DOMAINS.responseSignature,
  response_id: DOMAINS.responseId,
})) {
  assert(vectors.domains[name] === domain.subarray(0, domain.length - 1).toString("utf8"),
    `${name} domain changed`);
}
equalJson(generate(vectors.input), vectors.expected,
  "canonical runtime IPC recovery-plan reference");

if (args.has("--self-test")) {
  const changed = structuredClone(vectors.input);
  changed.approve_request_id_hex = "dc".repeat(32);
  assert(generate(changed).approve.request.cbor_hex !== vectors.expected.approve.request.cbor_hex,
    "request-ID mutation did not change canonical bytes");
  process.stdout.write("canonical_runtime_ipc_recovery_plan_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_runtime_ipc_recovery_plan_reference=verified\n");
