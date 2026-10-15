import { readFileSync } from "node:fs";
import process from "node:process";
import { blake3Hash } from "./lib/blake3-reference.mjs";

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
const hex = (value) => Buffer.from(value, "hex");
const digest = (domainHex, payloadHex) => blake3Hash(Buffer.concat([hex(domainHex), hex(payloadHex)]));
const equalHex = (actual, expected, label) => assert(actual.toString("hex") === expected, `${label} diverged`);

const workspace = process.argv[2] ?? new URL("..", import.meta.url);
const base = workspace instanceof URL ? workspace : new URL(`file:///${workspace.replaceAll("\\", "/")}/`);
const load = (name) => JSON.parse(readFileSync(new URL(name, base), "utf8"));

const official = load("BLAKE3-REFERENCE-VECTORS.json");
assert(official.schema_version === 1 && official.input_pattern === "byte_at_index_modulo_251",
  "official BLAKE3 vector metadata changed");
for (const test of official.cases) {
  const input = Buffer.alloc(test.input_len);
  for (let index = 0; index < input.length; index += 1) input[index] = index % 251;
  equalHex(blake3Hash(input), test.hash_hex, `official BLAKE3 case ${test.input_len}`);
}

const wire = load("CANONICAL-WIRE-VECTORS.json").acknowledgement_event.expected;
equalHex(digest(wire.event_id_domain_hex, wire.signed_event_cbor_hex), wire.event_id_hex,
  "canonical event ID");
const ratchet = load("CANONICAL-RATCHET-VECTORS.json").expected;
equalHex(digest(ratchet.prekey_pool_id_domain_hex, ratchet.prekey_pool.signed_cbor_hex),
  ratchet.prekey_pool_id_hex, "canonical prekey pool ID");
const recovery = load("CANONICAL-RECOVERY-VECTORS.json").expected;
equalHex(digest(recovery.recovery_id_domain_hex, recovery.recovery_identity_cbor_hex),
  recovery.recovery_id_hex, "canonical recovery ID");
for (const name of ["initial_checkpoint", "successor_checkpoint"]) {
  const checkpoint = recovery[name];
  equalHex(digest(checkpoint.checkpoint_id_domain_hex, checkpoint.signed_cbor_hex),
    checkpoint.checkpoint_id_hex, `${name} ID`);
}

console.log("blake3_reference_conformance=verified");
console.log(`official_vectors_checked=${official.cases.length}`);
console.log("kilogram_digest_vectors_checked=5");
console.log("independent_language=node-javascript");
console.log("runtime_crypto_changed=false");
