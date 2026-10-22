"""The independent computation of the custody v1 golden file, custody-v1.json.

Standard library only, so a reviewer can re-run it without installing anything:

    python3 -I backend/crates/adapter-hkdf/golden/custody_v1.py --check
    python3 -I backend/crates/adapter-hkdf/golden/custody_v1.py --write

--check recomputes every field this script owns and exits 1 on any difference from
the committed JSON. --write rewrites those fields and keeps the AEAD ciphertexts,
the only fields Rust computes (XChaCha20-Poly1305 is not in the standard library).

HKDF-SHA256 is built here from RFC 5869 (Extract, then Expand) over `hmac` and
`hashlib`, and checked against the RFC's test case 1 before anything is computed.
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

# The fields Rust computes; the script never owns them.
RUST_ONLY_FIELDS = ["envelopes[].ciphertext"]

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
    """RFC 5869 Appendix A.1, so the HKDF above is the RFC's."""
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


def compute(committed):
    """Every golden field; the ciphertexts are carried over from `committed`."""
    keys = compute_keys()
    committed_envelopes = committed.get("envelopes", []) if committed else []

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
    for index, envelope in enumerate(ENVELOPES):
        purpose = envelope["purpose"]
        key_id = keys[purpose]["key_id"].encode()
        subject_encoding = encode_parts(SUBJECTS[envelope["subject"]])
        associated_data = VERSION + lp(purpose.encode()) + lp(key_id) + lp(subject_encoding)
        header = VERSION + bytes([len(key_id)]) + key_id + envelope["nonce"]
        ciphertext = ""
        if index < len(committed_envelopes):
            ciphertext = committed_envelopes[index].get("ciphertext", "")
        envelopes.append(
            {
                "purpose": purpose,
                "subject": envelope["subject"],
                "nonce": envelope["nonce"].hex(),
                "plaintext": envelope["plaintext"].hex(),
                "associated_data": associated_data.hex(),
                "header": header.hex(),
                "ciphertext": ciphertext,
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
            "script": "golden/custody_v1.py computes every field but the rust_only ones",
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
    mode.add_argument("--write", action="store_true", help="rewrite the script's fields")
    arguments = parser.parse_args()

    check_rfc5869_test_case_1()
    committed = json.loads(GOLDEN_PATH.read_text()) if GOLDEN_PATH.exists() else None
    computed = compute(committed)

    if arguments.write:
        GOLDEN_PATH.write_text(json.dumps(computed, indent=2) + "\n")
        print(f"wrote {GOLDEN_PATH}")
        return 0

    if committed is None:
        print(f"no golden file at {GOLDEN_PATH}", file=sys.stderr)
        return 1
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
