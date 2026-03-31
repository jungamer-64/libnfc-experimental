/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartiere
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2025-2026 jungamer-64
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

#include "drivers/acr122-core.h"

#include <string.h>

struct acr122_usb_known_device {
  uint16_t vendor_id;
  uint16_t product_id;
  const char *name;
};

static const struct acr122_usb_known_device acr122_usb_devices[] = {
  {0x072F, 0x2200, "ACS ACR122"},
  {0x072F, 0x90CC, "Touchatag"},
  {0x072F, 0x2214, "ACS ACR1222"},
};

static const char *const acr122_pcsc_reader_prefixes[] = {
  "ACS ACR122",
  "ACS ACR 38U-CCID",
  "ACS ACR38U-CCID",
  "ACS AET65",
  "    CCID USB",
  NULL,
};

uint32_t
acr122_u32_to_le(uint32_t value)
{
  union {
    uint8_t bytes[4];
    uint32_t word;
  } encoded;

  encoded.bytes[0] = (uint8_t)(value & 0xFFu);
  encoded.bytes[1] = (uint8_t)((value >> 8) & 0xFFu);
  encoded.bytes[2] = (uint8_t)((value >> 16) & 0xFFu);
  encoded.bytes[3] = (uint8_t)((value >> 24) & 0xFFu);

  return encoded.word;
}

static size_t
acr122_build_prefixed_apdu(uint8_t *buffer, size_t buffer_size,
                           uint8_t ins, uint8_t p1, uint8_t p2,
                           uint8_t prefix, const uint8_t *data,
                           size_t data_len)
{
  if (data_len > UINT8_MAX - 1)
    return 0;
  if (buffer_size < sizeof(struct acr122_apdu_header) + data_len + 1)
    return 0;
  if (data == NULL && data_len != 0)
    return 0;

  struct acr122_apdu_header *header = (struct acr122_apdu_header *)buffer;
  header->bClass = ACR122_APDU_CLASS;
  header->bIns = ins;
  header->bP1 = p1;
  header->bP2 = p2;
  header->bLen = (uint8_t)(data_len + 1);

  uint8_t *payload = buffer + sizeof(struct acr122_apdu_header);
  payload[0] = prefix;
  if (data_len != 0)
    memcpy(payload + 1, data, data_len);

  return sizeof(struct acr122_apdu_header) + data_len + 1;
}

bool
acr122_is_usb_device(uint16_t vendor_id, uint16_t product_id)
{
  return acr122_usb_device_name(vendor_id, product_id) != NULL;
}

const char *
acr122_usb_device_name(uint16_t vendor_id, uint16_t product_id)
{
  for (size_t i = 0; i < sizeof(acr122_usb_devices) / sizeof(acr122_usb_devices[0]); i++) {
    if (acr122_usb_devices[i].vendor_id == vendor_id &&
        acr122_usb_devices[i].product_id == product_id) {
      return acr122_usb_devices[i].name;
    }
  }

  return NULL;
}

bool
acr122_is_pcsc_reader_name(const char *reader_name)
{
  if (reader_name == NULL)
    return false;

  for (size_t i = 0; acr122_pcsc_reader_prefixes[i] != NULL; i++) {
    size_t prefix_len = strlen(acr122_pcsc_reader_prefixes[i]);
    if (strncmp(acr122_pcsc_reader_prefixes[i], reader_name, prefix_len) == 0)
      return true;
  }

  return false;
}

size_t
acr122_build_apdu(uint8_t *buffer, size_t buffer_size,
                  uint8_t ins, uint8_t p1, uint8_t p2,
                  const uint8_t *data, size_t data_len, uint8_t le)
{
  if (buffer == NULL)
    return 0;
  if (buffer_size < sizeof(struct acr122_apdu_header))
    return 0;
  if (data_len > UINT8_MAX)
    return 0;
  if (data == NULL && data_len != 0)
    return 0;
  if (buffer_size < sizeof(struct acr122_apdu_header) + data_len)
    return 0;

  struct acr122_apdu_header *header = (struct acr122_apdu_header *)buffer;
  header->bClass = ACR122_APDU_CLASS;
  header->bIns = ins;
  header->bP1 = p1;
  header->bP2 = p2;

  if (data_len != 0) {
    header->bLen = (uint8_t)data_len;
    memcpy(buffer + sizeof(struct acr122_apdu_header), data, data_len);
  } else {
    header->bLen = le;
  }

  return sizeof(struct acr122_apdu_header) + data_len;
}

size_t
acr122_build_direct_transmit_apdu(uint8_t *buffer, size_t buffer_size,
                                  const uint8_t *payload, size_t payload_len)
{
  return acr122_build_prefixed_apdu(buffer, buffer_size,
                                    ACR122_APDU_INS_DIRECT_TRANSMIT, 0x00,
                                    0x00, ACR122_PN53X_HOST_TO_READER, payload,
                                    payload_len);
}

size_t
acr122_build_get_firmware_version_apdu(uint8_t *buffer, size_t buffer_size)
{
  return acr122_build_apdu(buffer, buffer_size, 0x00,
                           ACR122_APDU_P1_GET_FIRMWARE_VERSION, 0x00, NULL, 0,
                           0x00);
}

size_t
acr122_build_get_additional_data_apdu(uint8_t *buffer, size_t buffer_size,
                                      uint8_t le)
{
  return acr122_build_apdu(buffer, buffer_size,
                           ACR122_APDU_INS_GET_ADDITIONAL_DATA, 0x00, 0x00,
                           NULL, 0, le);
}

bool
acr122_parse_status_words(const uint8_t *status_bytes, size_t status_len,
                          struct acr122_status_word *status_word)
{
  if (status_bytes == NULL || status_word == NULL || status_len < 2)
    return false;

  status_word->sw1 = status_bytes[0];
  status_word->sw2 = status_bytes[1];
  status_word->has_more_data =
      status_word->sw1 == ACR122_SW1_MORE_DATA_AVAILABLE;
  status_word->more_data_length =
      status_word->has_more_data ? status_word->sw2 : 0;
  status_word->application_error =
      status_word->sw1 == ACR122_SW1_WARNING_WITH_NV_CHANGED &&
      status_word->sw2 == ACR122_SW2_PN53X_APPLICATION_LEVEL_ERROR;
  status_word->no_reply =
      status_word->sw1 == ACR122_SW1_WARNING_WITH_NV_CHANGED &&
      status_word->sw2 == 0x00;
  status_word->ok =
      status_word->sw1 == ACR122_SW1_SUCCESS &&
      status_word->sw2 == ACR122_SW2_SUCCESS;
  status_word->unexpected = !(status_word->has_more_data ||
                              status_word->application_error ||
                              status_word->no_reply ||
                              status_word->ok);

  return true;
}

bool
acr122_has_firmware_prefix(const char *firmware, const char *prefix)
{
  if (firmware == NULL || prefix == NULL)
    return false;

  const size_t prefix_len = strlen(prefix);
  return strncmp(firmware, prefix, prefix_len) == 0;
}

bool
acr122_is_acr122u_firmware(const char *firmware)
{
  return acr122_has_firmware_prefix(firmware, "ACR122U");
}

bool
acr122_is_acr122s_firmware(const char *firmware)
{
  return acr122_has_firmware_prefix(firmware, "ACR122S");
}
