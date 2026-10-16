# RFC-0193: Telegram-familiar UX, contextual identity and user-owned history

Status: accepted product direction; protocol mechanism pending.

## 1. Product direction

Kilogram should be immediately familiar to a Telegram user. Navigation, chat
list, conversation layout, composer, replies, forwards, reactions, media,
search, groups, channels and ordinary settings should use the same established
interaction vocabulary where the underlying semantics permit it. Kilogram does
not copy Telegram code, protected assets, branding or promise pixel identity;
it keeps its own visual identity and must not hide materially different
delivery or security semantics behind misleading UI.

The transport and storage model remains decentralized: direct P2P where policy
allows it, relay fallback, and bounded volunteer storage of opaque E2EE
ciphertext. Permanently readable history exists only in vaults controlled by
conversation participants. Infrastructure must not gain the ability to read or
silently rewrite it.

## 2. No permanent global social identifier

A stable recovery/account root can exist as an internal administrative key for
device authorization, encrypted account synchronization and recovery. It must
not become a network-visible universal social identifier carried into every
contact, room, channel, bot or public message.

The product-facing identity model must instead provide:

- pairwise identifiers for direct contacts;
- room- or channel-scoped participant identifiers;
- service-scoped identifiers for bots and external services;
- independently rotatable public profiles and discovery claims;
- explicit, informed linking when a user chooses to reuse one persona.

Possession of two identifiers from different scopes must not by itself prove
that they belong to one account. A root signature or certificate that exposes
the same root public key in every scope would violate this requirement.

The current canonical `AccountId` model is therefore an engineering precursor,
not the final public identity model. Before public groups, channels and bots are
declared production-compatible, their wire objects, indexes and moderation
interfaces must stop exposing a permanent cross-context Account ID. Existing
device and transport key separation remains necessary but is not sufficient.

This design cannot make already published content anonymous retroactively. An
observer may remember a username, avatar, writing style, timing, IP address or
an explicit persona link. The narrower guarantee is that Kilogram itself does
not require or reveal one permanent protocol identifier across unrelated
contexts.

## 3. User-owned received history

Once authenticated content has been delivered and committed to a participant's
vault, another participant cannot remotely overwrite or physically erase that
copy.

- An edit is a new signed event referencing the original. The normal chat view
  may show the newest version, while the local owner can inspect version
  history.
- A sender retraction is a signed request or tombstone. The normal view may
  collapse the message, but the recipient can reveal the retained original and
  the fact and time of retraction.
- No participant can remotely erase another participant's authored message.
- Physical local deletion is controlled by the owner of that local vault.
- A mutually requested purge may be implemented, but each device applies it
  only after its own owner's consent. A minimal audit/hash record may remain
  where causal integrity requires it.
- Disappearing-message behaviour must be agreed before delivery and described
  honestly: software cannot prevent a recipient from copying plaintext.

The UI may remain as uncluttered as Telegram, but it must not falsely claim that
remote content was irreversibly erased when a locally recoverable copy exists.

## 4. Abuse resistance without universal tracking

Free key generation makes global account uniqueness impossible. Kilogram must
not reintroduce a permanent global identifier merely to make bans, trials or
reputation convenient. Admission and rate limits are contextual:

- private conversations are capability/invitation based;
- each public room chooses its admission policy, slow start and posting quota;
- bans apply to the room credential or persona visible in that room;
- service-scoped stable pseudonyms may support one trial per existing persona
  without linking that persona to other services;
- creating a fresh root remains a Sybil escape, so stronger resistance needs a
  real cost or scarce credential: bounded proof of work, invitations/vouching,
  payment, privacy-preserving rate-limit tokens, or a combination;
- praise, complaints and reputation should be local or scope-limited by
  default, not one universal social score.

There is an unavoidable trade-off: cheap unlinkable identities make global
bans weak. Public communities must choose their admission cost rather than
silently turning every user into a permanently traceable account.

## 5. Threat boundary

E2EE and contextual identifiers do not protect plaintext on an unlocked or
compromised endpoint. Direct P2P can expose IP addresses; public content,
traffic timing and voluntarily reused profiles can also correlate people.
Local purge, compartmentalized personas, encrypted vaults and future relay
privacy reduce different risks, but none should be marketed as complete
anonymity.

The governing product principle is: the local client protects the interests of
its owner. Remote peers may request presentation changes, but they do not gain
retroactive control over the owner's already received information.
