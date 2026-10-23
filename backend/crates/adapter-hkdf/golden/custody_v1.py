"""The independent computation of the custody v1 golden file, custody-v1.json.

Standard library only, so a reviewer can re-run it without installing anything:

    python3 -I backend/crates/adapter-hkdf/golden/custody_v1.py --check
    python3 -I backend/crates/adapter-hkdf/golden/custody_v1.py --write

--check recomputes every field, the AEAD ciphertexts included, and exits 1 on any
difference from the committed JSON. --write rewrites the whole file from this script.

HKDF-SHA256 is built here from RFC 5869 (Extract, then Expand) over `hmac` and
`hashlib`, and checked against the RFC's test case 1 before anything is computed.

XChaCha20-Poly1305 is built here too, from integer arithmetic alone: the ChaCha20
block and Poly1305 from RFC 8439, HChaCha20 and the XChaCha construction from
draft-arciszewski-xchacha-03. Before anything is computed it must reproduce the
published vectors for Poly1305, HChaCha20, ChaCha20-Poly1305 and XChaCha20-Poly1305;
otherwise the script exits non-zero with a one-line reason.
"""

import argparse
import hashlib
import hmac
import json
import pathlib
import struct
import sys

GOLDEN_PATH = pathlib.Path(__file__).resolve().parent / "custody-v1.json"

# --- The door's constants ----------------------------------------------------------

VERSION = b"\x01"
SALT = b"zurfur/custody/v1"
KEY_INFO_PREFIX = b"zurfur/custody/v1/key/"
KEY_ID_INFO_PREFIX = b"zurfur/custody/v1/key-id/"
MAC_LABEL_PREFIX = b"zurfur/mac/v1/"
KEY_LENGTH = 32
FINGERPRINT_LENGTH = 8
NONCE_LENGTH = 24
AEAD_TAG_LENGTH = 16
MAC_TAG_LENGTH = 32
KEY_ID_SCHEME = "hkdf"

PURPOSES = ["did_custody", "signin_secrets", "den_mac"]
MAC_USES = ["den_node", "den_changelog_entry"]

# --- The fixed test inputs ---------------------------------------------------------

# An obviously-test root: the bytes 0x00 to 0x1f.
ROOT = bytes(range(32))

GOLDEN_DID = b"did:plc:goldentestalice222222222"

SUBJECTS = {
    "account_keys": [b"account_keys", GOLDEN_DID],
    "client_session": [b"client_session", GOLDEN_DID, b"golden-session-0001"],
    "auth_request": [b"auth_request", b"golden-state-0001"],
}

ENVELOPES = [
    {
        "purpose": "did_custody",
        "subject": "account_keys",
        "nonce": bytes(range(0x40, 0x58)),
        "plaintext": bytes(range(0x80, 0xE0)),
    },
    {
        "purpose": "signin_secrets",
        "subject": "client_session",
        "nonce": bytes(range(0x58, 0x70)),
        "plaintext": b'{"golden":"client session"}',
    },
    {
        "purpose": "signin_secrets",
        "subject": "auth_request",
        "nonce": bytes(range(0x70, 0x88)),
        "plaintext": b'{"golden":"auth request"}',
    },
]

MAC_MESSAGES = [
    {
        "use": "den_node",
        "parts": [b"node", b"0192f3a0-6b1c-7d2e-8f30-000000000001"],
    },
    {
        "use": "den_changelog_entry",
        "parts": [b"entry", b"0192f3a0-6b1c-7d2e-8f30-000000000002", b"42"],
    },
]

# The fields Rust computes; this script computes every field, so there are none.
RUST_ONLY_FIELDS = []

# --- The primitives ----------------------------------------------------------------


def hkdf_extract(salt, ikm):
    """RFC 5869 section 2.2: PRK = HMAC-Hash(salt, IKM)."""
    return hmac.new(salt, ikm, hashlib.sha256).digest()


def hkdf_expand(prk, info, length):
    """RFC 5869 section 2.3: T(i) = HMAC-Hash(PRK, T(i-1) | info | i), first L bytes."""
    output = b""
    block = b""
    counter = 1
    while len(output) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        output += block
        counter += 1
    return output[:length]


def hkdf(salt, ikm, info, length):
    return hkdf_expand(hkdf_extract(salt, ikm), info, length)


def lp(data):
    """A u32 big-endian length, then the bytes."""
    return struct.pack(">I", len(data)) + data


def encode_parts(parts):
    """The part count as a u32 big-endian, then each part length-prefixed."""
    return struct.pack(">I", len(parts)) + b"".join(lp(part) for part in parts)


def check_rfc5869_test_case_1():
    """RFC 5869 (IETF), Appendix A.1, Test Case 1: its inputs and expected PRK and OKM, so
    the HKDF above is the RFC's."""
    ikm = bytes([0x0B] * 22)
    salt = bytes(range(0x00, 0x0D))
    info = bytes(range(0xF0, 0xFA))
    prk = hkdf_extract(salt, ikm)
    okm = hkdf_expand(prk, info, 42)
    expected_prk = "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
    expected_okm = (
        "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        "34007208d5b887185865"
    )
    if prk.hex() != expected_prk or okm.hex() != expected_okm:
        sys.exit("this script's HKDF does not reproduce RFC 5869 test case 1")


WORD_MASK = 0xFFFFFFFF

# RFC 8439 section 2.3: the first four words of the state, "expand 32-byte k".
CHACHA20_CONSTANTS = [0x61707865, 0x3320646E, 0x79622D32, 0x6B206574]

CHACHA20_BLOCK_LENGTH = 64

# RFC 8439 section 2.5.1: the bits of r that are cleared ("clamped").
POLY1305_CLAMP = 0x0FFFFFFC0FFFFFFC0FFFFFFC0FFFFFFF

# RFC 8439 section 2.5.1: the prime 2^130 - 5.
POLY1305_PRIME = (1 << 130) - 5


def rotate_left(word, count):
    """RFC 8439 section 2.1: a 32-bit word rotated left by `count` bits."""
    return ((word << count) & WORD_MASK) | (word >> (32 - count))


def quarter_round(state, a, b, c, d):
    """RFC 8439 sections 2.1 and 2.2: the quarter round on four words of the state."""
    state[a] = (state[a] + state[b]) & WORD_MASK
    state[d] = rotate_left(state[d] ^ state[a], 16)
    state[c] = (state[c] + state[d]) & WORD_MASK
    state[b] = rotate_left(state[b] ^ state[c], 12)
    state[a] = (state[a] + state[b]) & WORD_MASK
    state[d] = rotate_left(state[d] ^ state[a], 8)
    state[c] = (state[c] + state[d]) & WORD_MASK
    state[b] = rotate_left(state[b] ^ state[c], 7)


def chacha20_rounds(state):
    """RFC 8439 section 2.3: 20 rounds, as 10 column rounds each followed by a diagonal round."""
    for _ in range(10):
        quarter_round(state, 0, 4, 8, 12)
        quarter_round(state, 1, 5, 9, 13)
        quarter_round(state, 2, 6, 10, 14)
        quarter_round(state, 3, 7, 11, 15)
        quarter_round(state, 0, 5, 10, 15)
        quarter_round(state, 1, 6, 11, 12)
        quarter_round(state, 2, 7, 8, 13)
        quarter_round(state, 3, 4, 9, 14)


def little_endian_words(data):
    """The bytes read as 32-bit little-endian words."""
    return [int.from_bytes(data[index : index + 4], "little") for index in range(0, len(data), 4)]


def serialize_words(words):
    """The 32-bit words written little-endian."""
    return b"".join(word.to_bytes(4, "little") for word in words)


def chacha20_block(key, counter, nonce):
    """RFC 8439 section 2.3: one 64-byte block from a 32-byte key, a counter and a 12-byte nonce."""
    key_words = little_endian_words(key)
    nonce_words = little_endian_words(nonce)
    initial_state = CHACHA20_CONSTANTS + key_words + [counter] + nonce_words
    state = list(initial_state)
    chacha20_rounds(state)
    added = [(word + initial) & WORD_MASK for word, initial in zip(state, initial_state)]
    return serialize_words(added)


def chacha20_encrypt(key, counter, nonce, plaintext):
    """RFC 8439 section 2.4: the plaintext XORed with the key stream from block `counter` on."""
    ciphertext = bytearray()
    for offset in range(0, len(plaintext), CHACHA20_BLOCK_LENGTH):
        key_stream = chacha20_block(key, counter + offset // CHACHA20_BLOCK_LENGTH, nonce)
        chunk = plaintext[offset : offset + CHACHA20_BLOCK_LENGTH]
        ciphertext += bytes(left ^ right for left, right in zip(chunk, key_stream))
    return bytes(ciphertext)


def poly1305_mac(key, message):
    """RFC 8439 section 2.5.1: the 16-byte tag of `message` under a 32-byte one-time key (r, s)."""
    r = int.from_bytes(key[:16], "little") & POLY1305_CLAMP
    s = int.from_bytes(key[16:32], "little")
    accumulator = 0
    for offset in range(0, len(message), 16):
        block = int.from_bytes(message[offset : offset + 16] + b"\x01", "little")
        accumulator = ((accumulator + block) * r) % POLY1305_PRIME
    return ((accumulator + s) % (1 << 128)).to_bytes(16, "little")


def poly1305_key_gen(key, nonce):
    """RFC 8439 section 2.6: the one-time Poly1305 key, the first 32 bytes of block 0."""
    return chacha20_block(key, 0, nonce)[:32]


def pad16(data):
    """RFC 8439 section 2.8: the zero bytes that bring `data` to a multiple of 16."""
    return bytes(-len(data) % 16)


def chacha20_poly1305_encrypt(key, nonce, associated_data, plaintext):
    """RFC 8439 section 2.8: the ciphertext, then its 16-byte tag, under a 12-byte nonce."""
    one_time_key = poly1305_key_gen(key, nonce)
    ciphertext = chacha20_encrypt(key, 1, nonce, plaintext)
    lengths = len(associated_data).to_bytes(8, "little") + len(ciphertext).to_bytes(8, "little")
    mac_data = associated_data + pad16(associated_data) + ciphertext + pad16(ciphertext) + lengths
    return ciphertext + poly1305_mac(one_time_key, mac_data)


def hchacha20(key, nonce):
    """draft-arciszewski-xchacha-03 section 2.2: a 32-byte subkey from a key and a 16-byte nonce."""
    state = CHACHA20_CONSTANTS + little_endian_words(key) + little_endian_words(nonce)
    chacha20_rounds(state)
    return serialize_words(state[0:4] + state[12:16])


def xchacha20_poly1305_encrypt(key, nonce, associated_data, plaintext):
    """draft-arciszewski-xchacha-03 section 2.3: ChaCha20-Poly1305 under the HChaCha20 subkey
    of the nonce's first 16 bytes, with four zero bytes and the nonce's last 8 as its nonce."""
    subkey = hchacha20(key, nonce[:16])
    chacha20_nonce = bytes(4) + nonce[16:24]
    return chacha20_poly1305_encrypt(subkey, chacha20_nonce, associated_data, plaintext)


# RFC 8439 (IETF), section 2.8.2: its key, associated data and plaintext, shared with the
# IETF Internet-Draft draft-arciszewski-xchacha-03, appendix A.1, as RustCrypto's
# chacha20poly1305-0.10.1/tests/lib.rs quotes them (KEY, AAD, PLAINTEXT).
AEAD_VECTOR_KEY = bytes.fromhex("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f")
AEAD_VECTOR_ASSOCIATED_DATA = bytes.fromhex("50515253c0c1c2c3c4c5c6c7")
AEAD_VECTOR_PLAINTEXT = (
    b"Ladies and Gentlemen of the class of '99: "
    b"If I could offer you only one tip for the future, sunscreen would be it."
)
AEAD_VECTOR_PLAINTEXT_LENGTH = 114


def check_poly1305_vector():
    """RFC 8439 (IETF), section 2.5.2, as RustCrypto's poly1305-0.8.0/tests/lib.rs quotes it
    (test_rfc7539_vector)."""
    key = bytes.fromhex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b")
    message = bytes.fromhex("43727970746f6772617068696320466f72756d2052657365617263682047726f7570")
    expected_tag = "a8061dc1305136c6c22b8baf0c0127a9"
    if poly1305_mac(key, message).hex() != expected_tag:
        sys.exit("this script's Poly1305 does not reproduce RFC 8439 section 2.5.2")


def check_hchacha20_vector():
    """The IETF Internet-Draft draft-arciszewski-xchacha-03, section 2.2.1, as RustCrypto's
    chacha20-0.9.1/src/xchacha.rs quotes it (module hchacha20_tests, test_vector)."""
    key = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
    nonce = bytes.fromhex("000000090000004a0000000031415927")
    expected_subkey = "82413b4227b27bfed30e42508a877d73a0f9e4d58a74a853c12ec41326d3ecdc"
    if hchacha20(key, nonce).hex() != expected_subkey:
        sys.exit("this script's HChaCha20 does not reproduce the XChaCha draft's section 2.2.1")


def check_chacha20_poly1305_vector():
    """RFC 8439 (IETF), section 2.8.2, as RustCrypto's chacha20poly1305-0.10.1/tests/lib.rs
    quotes it (module chacha20)."""
    nonce = bytes.fromhex("070000004041424344454647")
    expected_ciphertext = (
        "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6"
        "3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36"
        "92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc"
        "3ff4def08e4b7a9de576d26586cec64b6116"
    )
    expected_tag = "1ae10b594f09e26a7e902ecbd0600691"
    sealed = chacha20_poly1305_encrypt(
        AEAD_VECTOR_KEY, nonce, AEAD_VECTOR_ASSOCIATED_DATA, AEAD_VECTOR_PLAINTEXT
    )
    if sealed.hex() != expected_ciphertext + expected_tag:
        sys.exit("this script's ChaCha20-Poly1305 does not reproduce RFC 8439 section 2.8.2")


def check_xchacha20_poly1305_vector():
    """The IETF Internet-Draft draft-arciszewski-xchacha-03, appendix A.1, as RustCrypto's
    chacha20poly1305-0.10.1/tests/lib.rs quotes it (module xchacha20)."""
    nonce = bytes.fromhex("404142434445464748494a4b4c4d4e4f5051525354555657")
    expected_ciphertext = (
        "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb"
        "731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452"
        "2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9"
        "21f9664c97637da9768812f615c68b13b52e"
    )
    expected_tag = "c0875924c1c7987947deafd8780acf49"
    sealed = xchacha20_poly1305_encrypt(
        AEAD_VECTOR_KEY, nonce, AEAD_VECTOR_ASSOCIATED_DATA, AEAD_VECTOR_PLAINTEXT
    )
    if sealed.hex() != expected_ciphertext + expected_tag:
        sys.exit(
            "this script's XChaCha20-Poly1305 does not reproduce the XChaCha draft's appendix A.1"
        )


def check_aead_vectors():
    """The published vectors, so the XChaCha20-Poly1305 above is the RFC's and the draft's."""
    if len(AEAD_VECTOR_PLAINTEXT) != AEAD_VECTOR_PLAINTEXT_LENGTH:
        sys.exit("this script's copy of the RFC 8439 section 2.8.2 plaintext is not 114 bytes")
    check_poly1305_vector()
    check_hchacha20_vector()
    check_chacha20_poly1305_vector()
    check_xchacha20_poly1305_vector()


# --- The golden values -------------------------------------------------------------


def key_info(purpose):
    return KEY_INFO_PREFIX + purpose.encode()


def key_id_info(purpose):
    return KEY_ID_INFO_PREFIX + purpose.encode()


def mac_label(mac_use):
    return MAC_LABEL_PREFIX + mac_use.encode()


def label(text):
    return {"text": text.decode(), "hex": text.hex()}


def compute_keys():
    keys = {}
    for purpose in PURPOSES:
        key = hkdf(SALT, ROOT, key_info(purpose), KEY_LENGTH)
        fingerprint = hkdf(SALT, ROOT, key_id_info(purpose), FINGERPRINT_LENGTH)
        key_id = f"{KEY_ID_SCHEME}:{purpose}:{fingerprint.hex()}"
        keys[purpose] = {"key": key.hex(), "fingerprint": fingerprint.hex(), "key_id": key_id}
    return keys


def compute():
    """Every golden field, the AEAD ciphertexts included."""
    keys = compute_keys()

    constants = {
        "version": VERSION.hex(),
        "salt": label(SALT),
        "key_length": KEY_LENGTH,
        "key_id_fingerprint_length": FINGERPRINT_LENGTH,
        "nonce_length": NONCE_LENGTH,
        "aead_tag_length": AEAD_TAG_LENGTH,
        "mac_tag_length": MAC_TAG_LENGTH,
        "purposes": {
            purpose: {"info": label(key_info(purpose)), "key_id_info": label(key_id_info(purpose))}
            for purpose in PURPOSES
        },
        "mac_uses": {mac_use: {"label": label(mac_label(mac_use))} for mac_use in MAC_USES},
    }

    subjects = {
        name: {"parts": [part.hex() for part in parts], "encoding": encode_parts(parts).hex()}
        for name, parts in SUBJECTS.items()
    }

    envelopes = []
    for envelope in ENVELOPES:
        purpose = envelope["purpose"]
        key = bytes.fromhex(keys[purpose]["key"])
        key_id = keys[purpose]["key_id"].encode()
        subject_encoding = encode_parts(SUBJECTS[envelope["subject"]])
        associated_data = VERSION + lp(purpose.encode()) + lp(key_id) + lp(subject_encoding)
        header = VERSION + bytes([len(key_id)]) + key_id + envelope["nonce"]
        ciphertext = xchacha20_poly1305_encrypt(
            key, envelope["nonce"], associated_data, envelope["plaintext"]
        )
        envelopes.append(
            {
                "purpose": purpose,
                "subject": envelope["subject"],
                "nonce": envelope["nonce"].hex(),
                "plaintext": envelope["plaintext"].hex(),
                "associated_data": associated_data.hex(),
                "header": header.hex(),
                "ciphertext": ciphertext.hex(),
            }
        )

    den_key = bytes.fromhex(keys["den_mac"]["key"])
    mac_messages = []
    for message in MAC_MESSAGES:
        encoding = encode_parts(message["parts"])
        mac_input = lp(mac_label(message["use"])) + encoding
        tag = hmac.new(den_key, mac_input, hashlib.sha256).digest()
        mac_messages.append(
            {
                "use": message["use"],
                "parts": [part.hex() for part in message["parts"]],
                "encoding": encoding.hex(),
                "tag": tag.hex(),
            }
        )

    return {
        "provenance": {
            "script": "golden/custody_v1.py computes every field, the AEAD ciphertexts included",
            "rust_only": RUST_ONLY_FIELDS,
        },
        "constants": constants,
        "root": ROOT.hex(),
        "keys": keys,
        "subjects": subjects,
        "envelopes": envelopes,
        "mac_messages": mac_messages,
    }


def differences(expected, actual, path="$"):
    """The paths where `actual` differs from `expected`."""
    if isinstance(expected, dict) and isinstance(actual, dict):
        found = []
        for key in sorted(set(expected) | set(actual)):
            if key not in expected or key not in actual:
                found.append(f"{path}.{key}")
            else:
                found.extend(differences(expected[key], actual[key], f"{path}.{key}"))
        return found
    if isinstance(expected, list) and isinstance(actual, list):
        if len(expected) != len(actual):
            return [f"{path} (length)"]
        found = []
        for index, (left, right) in enumerate(zip(expected, actual)):
            found.extend(differences(left, right, f"{path}[{index}]"))
        return found
    return [] if expected == actual else [path]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="compare with the committed JSON")
    mode.add_argument("--write", action="store_true", help="rewrite the golden file")
    arguments = parser.parse_args()

    check_rfc5869_test_case_1()
    check_aead_vectors()
    computed = compute()

    if arguments.write:
        GOLDEN_PATH.write_text(json.dumps(computed, indent=2) + "\n")
        print(f"wrote {GOLDEN_PATH}")
        return 0

    if not GOLDEN_PATH.exists():
        print(f"no golden file at {GOLDEN_PATH}", file=sys.stderr)
        return 1
    committed = json.loads(GOLDEN_PATH.read_text())
    found = differences(computed, committed)
    if found:
        print("the golden file differs from this script at:", file=sys.stderr)
        for path in found:
            print(f"  {path}", file=sys.stderr)
        return 1
    print("the golden file matches this script")
    return 0


if __name__ == "__main__":
    sys.exit(main())
