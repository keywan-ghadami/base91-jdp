/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * The C ABI, used the way a caller would use it. Built and run by
 * `make -C rust/examples/c run`, and by CI, which is what keeps
 * include/base91z.h honest: a header that no compiler ever reads is a
 * document, not an interface.
 */

#include <stdio.h>
#include <string.h>

#include "base91z.h"

static int failures = 0;

static void check(int ok, const char *what) {
    printf("  %-58s %s\n", what, ok ? "ok" : "FAILED");
    if (!ok) failures++;
}

int main(void) {
    printf("base91z %s, specification %s, compressor %s\n\n",
           base91z_version(), base91z_spec_version(),
           base91z_has_compressor() ? "built in" : "absent");

    /* Round trip through the default entry point. */
    const char *json = "{\"user\":\"ada\",\"id\":42,\"role\":\"admin\"}";
    char *text = NULL;
    size_t text_len = 0;
    base91z_status st = base91z_encode((const uint8_t *)json, strlen(json),
                                       &text, &text_len);
    check(st == BASE91Z_OK, "encode returns OK");
    check(text != NULL && text[text_len] == '\0', "output is NUL-terminated past its length");
    check(strchr(text, '"') == NULL && strchr(text, '\\') == NULL,
          "output holds nothing a JSON string would escape");
    printf("    %zu bytes -> %zu characters: %s\n", strlen(json), text_len, text);

    uint8_t *back = NULL;
    size_t back_len = 0;
    st = base91z_decode(text, text_len, &back, &back_len, NULL);
    check(st == BASE91Z_OK, "decode returns OK");
    check(back_len == strlen(json) && memcmp(back, json, back_len) == 0,
          "decode returns the bytes that went in");
    base91z_free(text);
    base91z_free(back);

    /* The empty input is a real pointer, not a null one. */
    text = NULL;
    st = base91z_encode(NULL, 0, &text, &text_len);
    check(st == BASE91Z_OK && text != NULL && text_len == 0,
          "the empty input encodes to an empty, freeable string");
    base91z_free(text);

    /* A bad argument is reported, not crashed on. */
    st = base91z_encode((const uint8_t *)json, strlen(json), NULL, &text_len);
    check(st == BASE91Z_ERR_INVALID_ARGUMENT, "a null out-parameter is refused");
    st = base91z_encode(NULL, 7, &text, &text_len);
    check(st == BASE91Z_ERR_INVALID_ARGUMENT, "a null input with a non-zero length is refused");

    /* Malformed input: an error code, and where it was found. */
    size_t at = (size_t)-1;
    back = NULL;
    st = base91z_decode("\"not in the alphabet", 20, &back, &back_len, &at);
    check(st == BASE91Z_ERR_INVALID_CHARACTER, "a character outside the alphabet is refused");
    check(at == 0, "and the offset says where");
    check(back == NULL, "and nothing was allocated");
    printf("    %s at character %zu\n", base91z_strerror(st), at);

    /* The ceiling. Nine characters that ask for 65 536 bytes: a class-18
     * signal (a run of zero bytes) and a length field carrying 65536, which is
     * the largest that class may declare. This is the shape a budget exists
     * for -- the stream is nine characters and the output is 64 KiB. */
    const char *bomb = "m----Y]GA";
    back = NULL;
    st = base91z_decode_bounded(bomb, strlen(bomb), 1000, &back, &back_len, NULL);
    check(st == BASE91Z_ERR_INVALID_LENGTH, "a run past the budget is refused");
    st = base91z_decode_bounded(bomb, strlen(bomb), 1 << 20, &back, &back_len, NULL);
    check(st == BASE91Z_OK && back_len == 65536, "and produced in full when the budget allows");
    base91z_free(back);

    /* free(NULL) is a no-op, as everywhere else in C. */
    base91z_free(NULL);
    check(1, "base91z_free(NULL) is accepted");

    printf("\n%s\n", failures ? "FAILURES" : "all checks passed");
    return failures ? 1 : 0;
}
