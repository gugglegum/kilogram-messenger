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
  planId: "kilogram:history-recovery-plan-id:cbor-v1\0",
  planRecordId: "kilogram:runtime-ipc-recovery-plan-record-id:cbor-v1\0",
  checkpointSignature: "kilogram:history-recovery-checkpoint-signature:cbor-v1\0",
  schedulerSignature: "kilogram:runtime-ipc-recovery-scheduler-signature:cbor-v1\0",
  schedulerStateId: "kilogram:runtime-ipc-recovery-scheduler-state-id:cbor-v1\0",
  schedulerJitter: "kilogram:runtime-ipc-recovery-scheduler-jitter:cbor-v1\0",
  requestDigest: "kilogram:runtime-ipc-recovery-execution-request-digest:cbor-v1\0",
  responseSignature: "kilogram:runtime-ipc-recovery-execution-response-signature:cbor-v1\0",
  responseId: "kilogram:runtime-ipc-recovery-execution-response-id:cbor-v1\0",
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
const someUint = (value) => array([uint(1), uint(value)]);

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

function request(descriptorId, requestId, bearer, issuedAt, expiresAt, command) {
  const cbor = array([
    uint(VERSION), bytes(descriptorId), bytes(requestId), bytes(bearer),
    uint(issuedAt), uint(expiresAt), command,
  ]);
  return { cbor, digest: domainHash(DOMAINS.requestDigest, cbor) };
}

function checkpoint(input, signer, device) {
  const content = array([
    uint(VERSION),
    bytes(fromHex(input.account_id_hex, 32, "Account ID")),
    bytes(fromHex(input.conversation_id_hex, 32, "conversation ID")),
    bytes(fromHex(input.source_device_id_hex, 32, "source Device ID")),
    bytes(device),
    bytes(fromHex(input.confirmed_sas_hex, 32, "confirmed SAS")),
    uint(input.approved_range_start), uint(input.approved_range_end), uint(input.page_size),
    array([]), uint(input.approved_range_start), array([]),
  ]);
  return sign(DOMAINS.checkpointSignature, content, signer);
}

function schedulerContent(input, checkpointCbor, fields) {
  return array([
    uint(VERSION),
    bytes(fromHex(input.plan_id_hex, 32, "plan ID")),
    bytes(fromHex(input.recipient_device_id_hex, 32, "recipient Device ID")),
    uint(fields.generation),
    fields.previousStateId === undefined ? none() : someBytes(fields.previousStateId),
    uint(fields.transition), uint(fields.lifecycle), uint(fields.totalAttempts),
    uint(fields.consecutiveFailures), uint(fields.lastObserved), uint(fields.nextAttemptAt),
    uint(fields.scheduledDelay), uint(input.retry_base_seconds), uint(input.retry_max_seconds),
    uint(input.attempt_lease_seconds), bytes(checkpointCbor),
    fields.failureCode === undefined ? none() : someUint(fields.failureCode),
  ]);
}

function signedScheduler(input, checkpointCbor, fields, signer) {
  const signed = sign(DOMAINS.schedulerSignature,
    schedulerContent(input, checkpointCbor, fields), signer);
  return { ...signed, id: domainHash(DOMAINS.schedulerStateId, signed.cbor) };
}

function schedulerJson(value) {
  return {
    content_cbor_hex: value.content.toString("hex"),
    signing_preimage_hex: value.preimage.toString("hex"),
    signature_hex: value.signature.toString("hex"),
    signed_cbor_hex: value.cbor.toString("hex"),
    state_id_hex: value.id.toString("hex"),
  };
}

function backoffDelay(input, failureExponent, attemptOrdinal) {
  const shift = Math.min(Math.max(failureExponent - 1, 0), 63);
  const exponential = Math.min(input.retry_base_seconds * (2 ** shift), input.retry_max_seconds);
  const floor = Math.floor((exponential + 1) / 2);
  const span = exponential - floor + 1;
  const exponent = Buffer.alloc(4); exponent.writeUInt32LE(failureExponent);
  const ordinal = Buffer.alloc(8); ordinal.writeBigUInt64LE(BigInt(attemptOrdinal));
  const sample = blake3Hash(Buffer.concat([
    DOMAINS.schedulerJitter, fromHex(input.plan_id_hex, 32, "plan ID"), exponent, ordinal,
  ])).readBigUInt64LE(0);
  return floor + Number(sample % BigInt(span));
}

function snapshot(input, scheduler, observedAt, network, power, readiness) {
  return array([
    uint(VERSION), bytes(fromHex(input.plan_state_cbor_hex, undefined, "plan state")),
    bytes(scheduler.cbor), uint(observedAt), uint(network), uint(power), readiness,
    uint(0), uint(0), uint(0),
  ]);
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
  const descriptorId = fromHex(input.descriptor_id_hex, 32, "descriptor ID");
  const bearer = fromHex(input.bearer_token_hex, 32, "bearer token");
  const signedPlan = fromHex(input.signed_plan_cbor_hex, undefined, "signed plan");
  const planId = domainHash(DOMAINS.planId, signedPlan);
  assert(planId.equals(fromHex(input.plan_id_hex, 32, "plan ID")), "plan ID changed");
  const planRecordId = domainHash(DOMAINS.planRecordId,
    array([uint(VERSION), bytes(signedPlan), none()]));
  assert(planRecordId.equals(fromHex(input.plan_record_id_hex, 32, "plan record ID")),
    "plan record ID changed");
  const expectedPlanState = array([
    uint(VERSION), bytes(planRecordId), uint(0), bytes(signedPlan), none(),
  ]);
  assert(expectedPlanState.equals(fromHex(input.plan_state_cbor_hex, undefined, "plan state")),
    "approved plan state changed");

  const initialCheckpoint = checkpoint(input, signer, device);
  const initialized = signedScheduler(input, initialCheckpoint.cbor, {
    generation: 0, transition: 0, lifecycle: 0, totalAttempts: 0,
    consecutiveFailures: 0, lastObserved: input.start_issued_at_unix_seconds,
    nextAttemptAt: input.start_issued_at_unix_seconds, scheduledDelay: 0,
  }, signer);
  const started = signedScheduler(input, initialCheckpoint.cbor, {
    generation: 1, previousStateId: initialized.id, transition: 1, lifecycle: 1,
    totalAttempts: 1, consecutiveFailures: 0,
    lastObserved: input.start_issued_at_unix_seconds,
    nextAttemptAt: input.start_issued_at_unix_seconds + input.attempt_lease_seconds,
    scheduledDelay: input.attempt_lease_seconds,
  }, signer);
  const failedDelay = backoffDelay(input, 1, 1);
  const failed = signedScheduler(input, initialCheckpoint.cbor, {
    generation: 2, previousStateId: started.id, transition: 2, lifecycle: 0,
    totalAttempts: 1, consecutiveFailures: 1,
    lastObserved: input.status_issued_at_unix_seconds,
    nextAttemptAt: input.status_issued_at_unix_seconds + failedDelay,
    scheduledDelay: failedDelay, failureCode: input.failure_code,
  }, signer);

  const startRequest = request(
    descriptorId, fromHex(input.start_request_id_hex, 32, "start request ID"), bearer,
    input.start_issued_at_unix_seconds, input.start_expires_at_unix_seconds,
    array([uint(0), bytes(planId), bytes(planRecordId), uint(input.retry_base_seconds),
      uint(input.retry_max_seconds), uint(input.attempt_lease_seconds)]),
  );
  const statusRequest = request(
    descriptorId, fromHex(input.status_request_id_hex, 32, "status request ID"), bearer,
    input.status_issued_at_unix_seconds, input.status_expires_at_unix_seconds,
    array([uint(1), bytes(planId)]),
  );
  const startResponse = response(
    descriptorId, fromHex(input.start_request_id_hex, 32, "start request ID"),
    startRequest.digest, device,
    array([uint(0), uint(0), snapshot(input, initialized,
      input.start_issued_at_unix_seconds, 0, 0, array([uint(0)]))]), signer,
  );
  const statusResponse = response(
    descriptorId, fromHex(input.status_request_id_hex, 32, "status request ID"),
    statusRequest.digest, device,
    array([uint(1), snapshot(input, failed,
      input.status_issued_at_unix_seconds, 1, 1,
      array([uint(1), uint(input.status_issued_at_unix_seconds + failedDelay)]))]), signer,
  );

  return {
    initial_checkpoint: {
      content_cbor_hex: initialCheckpoint.content.toString("hex"),
      signing_preimage_hex: initialCheckpoint.preimage.toString("hex"),
      signature_hex: initialCheckpoint.signature.toString("hex"),
      signed_cbor_hex: initialCheckpoint.cbor.toString("hex"),
    },
    initialized_scheduler: schedulerJson(initialized),
    started_scheduler: schedulerJson(started),
    failed_scheduler: {
      scheduled_delay_seconds: failedDelay,
      next_attempt_at_unix_seconds: input.status_issued_at_unix_seconds + failedDelay,
      state: schedulerJson(failed),
    },
    start: caseJson(startRequest, startResponse),
    status: caseJson(statusRequest, statusResponse),
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
  schema.covered_commands.length === 38 && schema.covered_results.length === 37,
  "runtime IPC schema registry counts changed");
assert(vectors.schema_version === 1 && vectors.milestone === "M1.1.44" &&
  vectors.status === "candidate-not-active" && vectors.changes_active_runtime_ipc === false,
  "recovery-execution vector header changed");
for (const [name, domain] of Object.entries({
  scheduler_signature: DOMAINS.schedulerSignature,
  scheduler_state_id: DOMAINS.schedulerStateId,
  scheduler_jitter: DOMAINS.schedulerJitter,
  request_digest: DOMAINS.requestDigest,
  response_signature: DOMAINS.responseSignature,
  response_id: DOMAINS.responseId,
})) {
  assert(vectors.domains[name] === domain.subarray(0, domain.length - 1).toString("utf8"),
    `${name} domain changed`);
}
equalJson(generate(vectors.input), vectors.expected,
  "canonical runtime IPC recovery-execution reference");

if (args.has("--self-test")) {
  const changed = structuredClone(vectors.input);
  changed.start_request_id_hex = "dc".repeat(32);
  assert(generate(changed).start.request.cbor_hex !== vectors.expected.start.request.cbor_hex,
    "request-ID mutation did not change canonical bytes");
  const changedFailure = structuredClone(vectors.input);
  changedFailure.failure_code = 8;
  assert(generate(changedFailure).failed_scheduler.state.signed_cbor_hex !==
    vectors.expected.failed_scheduler.state.signed_cbor_hex,
  "failure-code mutation did not change signed scheduler state");
  process.stdout.write("canonical_runtime_ipc_recovery_execution_reference_self_test=passed\n");
}
process.stdout.write("independent_blake3=verified\n");
process.stdout.write("canonical_runtime_ipc_recovery_execution_reference=verified\n");
