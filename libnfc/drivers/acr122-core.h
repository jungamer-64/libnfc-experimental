/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2026      OpenAI
 * See AUTHORS file for a more comprehensive list of contributors.
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

/**
 * @file acr122-core.h
 * @brief Internal helpers shared by ACR122 transport drivers
 */

#ifndef __NFC_DRIVER_ACR122_CORE_H__
#define __NFC_DRIVER_ACR122_CORE_H__

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
  ACR122_CCID_PC_TO_RDR_ICC_POWER_ON = 0x62,
  ACR122_CCID_PC_TO_RDR_ICC_POWER_OFF = 0x63,
  ACR122_CCID_PC_TO_RDR_XFR_BLOCK = 0x6F,

  ACR122_CCID_RDR_TO_PC_DATABLOCK = 0x80,
  ACR122_CCID_RDR_TO_PC_SLOTSTATUS = 0x81,
};

enum {
  ACR122_APDU_CLASS = 0xFF,
  ACR122_APDU_INS_DIRECT_TRANSMIT = 0x00,
  ACR122_APDU_INS_GET_ADDITIONAL_DATA = 0xC0,
  ACR122_APDU_P1_GET_FIRMWARE_VERSION = 0x48,
  ACR122_PN53X_HOST_TO_READER = 0xD4,
  ACR122_PN53X_READER_TO_HOST = 0xD5,
  ACR122_SW1_MORE_DATA_AVAILABLE = 0x61,
  ACR122_SW1_WARNING_WITH_NV_CHANGED = 0x63,
  ACR122_SW1_SUCCESS = 0x90,
  ACR122_SW2_SUCCESS = 0x00,
  ACR122_SW2_PN53X_APPLICATION_LEVEL_ERROR = 0x7F,
};

#pragma pack(push, 1)

struct acr122_ccid_header {
  uint8_t bMessageType;
  uint32_t dwLength;
  uint8_t bSlot;
  uint8_t bSeq;
  uint8_t bMessageSpecific[3];
};

struct acr122_apdu_header {
  uint8_t bClass;
  uint8_t bIns;
  uint8_t bP1;
  uint8_t bP2;
  uint8_t bLen;
};

#pragma pack(pop)

struct acr122_status_word {
  uint8_t sw1;
  uint8_t sw2;
  uint8_t more_data_length;
  bool has_more_data;
  bool application_error;
  bool no_reply;
  bool ok;
  bool unexpected;
};

uint32_t acr122_u32_to_le(uint32_t value);

bool acr122_is_usb_device(uint16_t vendor_id, uint16_t product_id);
const char *acr122_usb_device_name(uint16_t vendor_id, uint16_t product_id);

bool acr122_is_pcsc_reader_name(const char *reader_name);

size_t acr122_build_apdu(uint8_t *buffer, size_t buffer_size,
                         uint8_t ins, uint8_t p1, uint8_t p2,
                         const uint8_t *data, size_t data_len, uint8_t le);
size_t acr122_build_direct_transmit_apdu(uint8_t *buffer, size_t buffer_size,
                                         const uint8_t *payload,
                                         size_t payload_len);
size_t acr122_build_get_firmware_version_apdu(uint8_t *buffer,
                                              size_t buffer_size);
size_t acr122_build_get_additional_data_apdu(uint8_t *buffer,
                                             size_t buffer_size, uint8_t le);

bool acr122_parse_status_words(const uint8_t *status_bytes, size_t status_len,
                               struct acr122_status_word *status_word);

bool acr122_has_firmware_prefix(const char *firmware, const char *prefix);
bool acr122_is_acr122u_firmware(const char *firmware);
bool acr122_is_acr122s_firmware(const char *firmware);

#endif // ! __NFC_DRIVER_ACR122_CORE_H__
