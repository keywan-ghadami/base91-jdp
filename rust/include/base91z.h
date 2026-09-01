/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * Base91z: a binary-to-text encoding for bytes inside text protocols, on an
 * alphabet JSON never has to escape, with typed segments and zstd inside.
 *
 * Specification v0.4.0 (final). This header declares the C ABI of the Rust
 * library; the implementation is src/ffi.rs, and there is no separate C
 * implementation to keep in step with it.
 *
 *   cc app.c -I rust/include -L rust/target/release -lbase91z
 *
 * Link either target/release/libbase91z.so or libbase91z.a from
 * `cargo build --release`. The static library carries the Rust runtime and
 * needs no other flags on Linux beyond -lm -lpthread -ldl.
 *
 * OWNERSHIP. Every out-parameter that receives a buffer receives one this
 * library allocated with malloc(). Release it with base91z_free(). On any
 * error the out-parameters are left untouched, so a caller may initialise
 * them to NULL once and free unconditionally.
 *
 * THREADS. No global state, nothing retained across a call: every function
 * here may be called concurrently on separate data.
 */

#ifndef BASE91Z_H
#define BASE91Z_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Zero is success and every failure is positive, so `if (status)` reads as
 * "if it failed". Codes 1..12 are the decode conditions of specification
 * section 13, in its order; 13..15 belong to this boundary. */
typedef enum {
    BASE91Z_OK = 0,
    BASE91Z_ERR_INVALID_CHARACTER = 1,
    BASE91Z_ERR_UNEXPECTED_EOS = 2,
    BASE91Z_ERR_UNKNOWN_CLASS = 3,
    BASE91Z_ERR_EXTENDED_CLASS = 4,
    BASE91Z_ERR_INVALID_FLUSH = 5,
    BASE91Z_ERR_INVALID_PARAMS = 6,
    BASE91Z_ERR_INVALID_LENGTH = 7,
    BASE91Z_ERR_INVALID_FINAL_BLOCK = 8,
    BASE91Z_ERR_INVALID_INDEX = 9,
    BASE91Z_ERR_INVALID_RUN_VALUE = 10,
    BASE91Z_ERR_MALFORMED_PADDING = 11,
    BASE91Z_ERR_MALFORMED_FRAME = 12,
    BASE91Z_ERR_ALLOC = 13,
    BASE91Z_ERR_INVALID_ARGUMENT = 14,
    BASE91Z_ERR_NO_COMPRESSOR = 15
} base91z_status;

/* The default compression level, which base91z_encode uses. */
#define BASE91Z_DEFAULT_LEVEL 1

/* Bounds a decoder enforces per class (specification section 11.4). A caller
 * sizing its own budget for base91z_decode_bounded may find them useful. */
#define BASE91Z_MAX_SEGMENT_BYTES     65536u
#define BASE91Z_MAX_FRAME_BYTES       16777216u
#define BASE91Z_MAX_BLOCK_BYTES       131072u
#define BASE91Z_MAX_FRAME_PLAIN_BYTES 67108864u

/* Encode, compressing where that is smaller. This is the entry point:
 * compression is part of the format rather than a stage in front of it, and
 * base91z_decode reads whatever it chose. Total -- every byte sequence has an
 * encoding -- so it fails only on a bad argument or on the allocation.
 *
 * On BASE91Z_OK, *out_str is a NUL-terminated buffer the caller frees with
 * base91z_free(), and *out_len its length excluding the terminator.
 *
 * `data` may be NULL only when data_len is 0. */
base91z_status base91z_encode(const uint8_t *data, size_t data_len,
                              char **out_str, size_t *out_len);

/* Encode at an explicit level. Negative is faster and larger, higher is
 * slower and smaller.
 *
 * The level is part of the encoding and not of the payload: the same bytes at
 * two levels give two different strings, both valid, both decoding back to
 * those bytes. Do not use the text as a signature input, a cache key or a
 * fixture -- compute those over the payload.
 *
 * Returns BASE91Z_ERR_NO_COMPRESSOR if built without the zstd feature. */
base91z_status base91z_encode_at(const uint8_t *data, size_t data_len, int level,
                                 char **out_str, size_t *out_len);

/* Encode without compressing: the container and every typed class except the
 * compressed ones. No level, so within one version of this library the same
 * bytes always give the same string. */
base91z_status base91z_encode_plain(const uint8_t *data, size_t data_len,
                                    char **out_str, size_t *out_len);

/* Decode, with no ceiling of the caller's own.
 *
 * On BASE91Z_OK, *out_data is a buffer of *out_len bytes, freed with
 * base91z_free(); it is not NUL-terminated, and is non-NULL even when the
 * length is 0. out_error_at may be NULL; otherwise it receives the character
 * offset at which a failure was detected, and is written only on failure.
 *
 * `s` need not be NUL-terminated: exactly s_len bytes are read, one character
 * per byte. It may be NULL only when s_len is 0.
 *
 * FOR INPUT YOU DID NOT PRODUCE, PREFER base91z_decode_bounded. A length
 * field is four characters and can ask for far more memory than the stream is
 * long. */
base91z_status base91z_decode(const char *s, size_t s_len,
                              uint8_t **out_data, size_t *out_len,
                              size_t *out_error_at);

/* Decode, refusing to produce more than max_bytes.
 *
 * The ceiling is checked before anything is reserved for the output, and a
 * stream that would cross it is refused with BASE91Z_ERR_INVALID_LENGTH
 * rather than truncated -- so a short read is never mistaken for a short
 * payload (specification section 16). */
base91z_status base91z_decode_bounded(const char *s, size_t s_len, size_t max_bytes,
                                      uint8_t **out_data, size_t *out_len,
                                      size_t *out_error_at);

/* Release a buffer this library returned. NULL is accepted and ignored.
 *
 * free() is the same thing wherever caller and library share a C runtime;
 * this exists for where they do not, and costs nothing to prefer. */
void base91z_free(void *p);

/* A static, human-readable description of a status. Never freed. */
const char *base91z_strerror(base91z_status status);

/* The specification version implemented, e.g. "0.4.0". Never freed. */
const char *base91z_spec_version(void);

/* This library's own version. Never freed. */
const char *base91z_version(void);

/* Whether the compressed classes (17 and 20) were built in. Without them
 * encoding still works, using the container alone, and decoding refuses a
 * stream that contains one. */
bool base91z_has_compressor(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* BASE91Z_H */
