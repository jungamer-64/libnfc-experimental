/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Copyright (C) 2025-2026 jungamer-64
 * See AUTHORS file for a more comprehensive list of contributors.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *  1) Redistributions of source code must retain the above copyright notice,
 *  this list of conditions and the following disclaimer.
 *  2 )Redistributions in binary form must reproduce the above copyright
 *  notice, this list of conditions and the following disclaimer in the
 *  documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
 * LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 *
 * Note that this license only applies on the examples, NFC library itself is under LGPL
 *
 */

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <nfc/nfc.h>

int main(void)
{
    nfc_context *context = NULL;
    uint8_t data[4] = {0x01, 0x02, 0x03, 0x04};
    uint8_t crc[2] = {0x00, 0x00};
    static const uint8_t expected_crc[2] = {0x91, 0x39};
    const char *version;

    nfc_init(&context);
    iso14443b_crc(data, sizeof(data), crc);

    if (memcmp(crc, expected_crc, sizeof(expected_crc)) != 0)
    {
        fprintf(stderr, "Unexpected CRC_B bytes: %02x%02x\n", crc[0], crc[1]);
        if (context)
        {
            nfc_exit(context);
        }
        return 1;
    }

    version = nfc_version();
    if (version == NULL || version[0] == '\0')
    {
        fprintf(stderr, "Expected nfc_version() to return a non-empty string\n");
        if (context)
        {
            nfc_exit(context);
        }
        return 2;
    }

    if (context)
    {
        nfc_exit(context);
    }

    printf("ffi-sanity OK: version='%s' crc=%02x%02x\n", version, crc[0], crc[1]);
    return 0;
}
