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

/**
 * @file acr122_pcsc.c
 * @brief Driver for ACR122 devices (e.g. Tikitag, Touchatag, ACS ACR122) behind PC/SC
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif // HAVE_CONFIG_H

#include <errno.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <nfc/nfc.h>

#include "chips/pn53x.h"
#include "drivers/acr122-core.h"
#include "drivers/acr122_pcsc.h"
#include "nfc-internal.h"
#include "nfc-secure.h"

#ifdef __APPLE__
#include <PCSC/winscard.h>
#include <PCSC/wintypes.h>
#else
#include <winscard.h>
#endif

#define ACR122_PCSC_DRIVER_NAME "acr122_pcsc"

#if defined(_WIN32)
#define IOCTL_CCID_ESCAPE_SCARD_CTL_CODE SCARD_CTL_CODE(3500)
#elif defined(__APPLE__)
#define IOCTL_CCID_ESCAPE_SCARD_CTL_CODE (((0x31) << 16) | ((3500) << 2))
#elif defined(__FreeBSD__) || defined(__OpenBSD__) || defined(__NetBSD__)
#define IOCTL_CCID_ESCAPE_SCARD_CTL_CODE (((0x31) << 16) | ((3500) << 2))
#elif defined(__linux__)
#include <reader.h>
#define IOCTL_CCID_ESCAPE_SCARD_CTL_CODE SCARD_CTL_CODE(1)
#else
#error "Can't determine serial string for your system"
#endif

#define SCARD_OPERATION_SUCCESS 0x61
#define SCARD_OPERATION_ERROR 0x63

#ifndef SCARD_PROTOCOL_UNDEFINED
#define SCARD_PROTOCOL_UNDEFINED SCARD_PROTOCOL_UNSET
#endif

#define ACR122_PCSC_WRAP_LEN 6
#define ACR122_PCSC_COMMAND_LEN 266
#define ACR122_PCSC_RESPONSE_LEN 268

#define LOG_GROUP NFC_LOG_GROUP_DRIVER
#define LOG_CATEGORY "libnfc.driver.acr122_pcsc"

const struct pn53x_io acr122_pcsc_io;

static char *acr122_pcsc_firmware(nfc_device *pnd);

struct acr122_pcsc_data {
  SCARDHANDLE hCard;
  SCARD_IO_REQUEST ioCard;
  uint8_t abtRx[ACR122_PCSC_RESPONSE_LEN];
  size_t szRx;
};

#define DRIVER_DATA(pnd) ((struct acr122_pcsc_data *)(pnd->driver_data))

static SCARDCONTEXT _SCardContext;
static int _iSCardContextRefCount = 0;

static SCARDCONTEXT *
acr122_pcsc_get_scardcontext(void)
{
  if (_iSCardContextRefCount == 0) {
    if (SCardEstablishContext(SCARD_SCOPE_USER, NULL, NULL, &_SCardContext) !=
        SCARD_S_SUCCESS) {
      return NULL;
    }
  }
  _iSCardContextRefCount++;

  return &_SCardContext;
}

static void
acr122_pcsc_free_scardcontext(void)
{
  if (_iSCardContextRefCount) {
    _iSCardContextRefCount--;
    if (!_iSCardContextRefCount)
      SCardReleaseContext(_SCardContext);
  }
}

#define PCSC_MAX_DEVICES 16

static bool
acr122_pcsc_copy_connstring(const nfc_connstring source,
                            nfc_connstring destination)
{
  size_t length = nfc_safe_strlen(source, sizeof(nfc_connstring));
  if (length >= sizeof(nfc_connstring))
    return false;
  if (nfc_safe_memcpy(destination, sizeof(nfc_connstring), source, length) < 0)
    return false;
  destination[length] = '\0';
  return true;
}

static bool
acr122_pcsc_parse_device_index(const char *value, size_t *index)
{
  if (value == NULL || index == NULL)
    return false;

  errno = 0;
  char *end = NULL;
  unsigned long parsed = strtoul(value, &end, 10);
  if (errno != 0 || end == value || *end != '\0')
    return false;

  *index = (size_t)parsed;
  return true;
}

static size_t
acr122_pcsc_scan(const nfc_context *context, nfc_connstring connstrings[],
                 const size_t connstrings_len)
{
  (void)context;
  size_t szPos = 0;
  char acDeviceNames[256 + 64 * PCSC_MAX_DEVICES];
  const size_t szDeviceNamesLen = sizeof(acDeviceNames);
  SCARDCONTEXT *pscc;

  if (nfc_secure_memset(acDeviceNames, '\0', szDeviceNamesLen) < 0)
    return 0;

  if (!(pscc = acr122_pcsc_get_scardcontext())) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "Warning: %s",
            "PCSC context not found (make sure PCSC daemon is running).");
    return 0;
  }

  DWORD dwDeviceNamesLen = (DWORD)szDeviceNamesLen;
  if (SCardListReaders(*pscc, NULL, acDeviceNames, &dwDeviceNamesLen) !=
      SCARD_S_SUCCESS) {
    acr122_pcsc_free_scardcontext();
    return 0;
  }

  size_t device_found = 0;
  while ((acDeviceNames[szPos] != '\0') && (device_found < connstrings_len)) {
    if (acr122_is_pcsc_reader_name(acDeviceNames + szPos)) {
      snprintf(connstrings[device_found], sizeof(nfc_connstring), "%s:%s",
               ACR122_PCSC_DRIVER_NAME, acDeviceNames + szPos);
      device_found++;
    } else {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
              "PCSC device [%s] is not supported by acr122_pcsc.",
              acDeviceNames + szPos);
    }

    while (acDeviceNames[szPos++] != '\0')
      ;
  }

  acr122_pcsc_free_scardcontext();
  return device_found;
}

struct acr122_pcsc_descriptor {
  char *pcsc_device_name;
};

static nfc_device *
acr122_pcsc_open(const nfc_context *context, const nfc_connstring connstring)
{
  struct acr122_pcsc_descriptor ndd = {0};
  nfc_connstring fullconnstring;
  nfc_device *pnd = NULL;
  bool context_acquired = false;
  bool card_connected = false;

  int connstring_decode_level =
      connstring_decode(connstring, ACR122_PCSC_DRIVER_NAME, "pcsc",
                        &ndd.pcsc_device_name, NULL);
  if (connstring_decode_level < 1)
    goto error;

  if (connstring_decode_level == 1) {
    size_t found = acr122_pcsc_scan(context, &fullconnstring, 1);
    if (found < 1)
      goto error;
    free(ndd.pcsc_device_name);
    ndd.pcsc_device_name = NULL;
    connstring_decode_level =
        connstring_decode(fullconnstring, ACR122_PCSC_DRIVER_NAME, "pcsc",
                          &ndd.pcsc_device_name, NULL);
    if (connstring_decode_level < 2)
      goto error;
  } else {
    if (!acr122_pcsc_copy_connstring(connstring, fullconnstring))
      goto error;
  }

  if (nfc_safe_strlen(ndd.pcsc_device_name, 256) < 5) {
    size_t index = 0;
    if (!acr122_pcsc_parse_device_index(ndd.pcsc_device_name, &index))
      goto error;

    nfc_connstring *connstrings = malloc(sizeof(nfc_connstring) * (index + 1));
    if (!connstrings) {
      perror("malloc");
      goto error;
    }

    size_t found = acr122_pcsc_scan(context, connstrings, index + 1);
    if (found <= index || !acr122_pcsc_copy_connstring(connstrings[index],
                                                       fullconnstring)) {
      free(connstrings);
      goto error;
    }
    free(connstrings);

    free(ndd.pcsc_device_name);
    ndd.pcsc_device_name = NULL;
    connstring_decode_level =
        connstring_decode(fullconnstring, ACR122_PCSC_DRIVER_NAME, "pcsc",
                          &ndd.pcsc_device_name, NULL);
    if (connstring_decode_level < 2)
      goto error;
  }

  pnd = nfc_device_new(context, fullconnstring);
  if (!pnd) {
    perror("malloc");
    goto error;
  }

  pnd->driver_data = malloc(sizeof(struct acr122_pcsc_data));
  if (!pnd->driver_data) {
    perror("malloc");
    goto error;
  }

  if (pn53x_data_new(pnd, &acr122_pcsc_io) == NULL) {
    perror("malloc");
    goto error;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Attempt to open %s",
          ndd.pcsc_device_name);

  SCARDCONTEXT *pscc = acr122_pcsc_get_scardcontext();
  if (pscc == NULL)
    goto error;
  context_acquired = true;

  if (SCardConnect(*pscc, ndd.pcsc_device_name, SCARD_SHARE_EXCLUSIVE,
                   SCARD_PROTOCOL_T0 | SCARD_PROTOCOL_T1,
                   &(DRIVER_DATA(pnd)->hCard),
                   (void *)&(DRIVER_DATA(pnd)->ioCard.dwProtocol)) !=
      SCARD_S_SUCCESS) {
    if (SCardConnect(*pscc, ndd.pcsc_device_name, SCARD_SHARE_DIRECT, 0,
                     &(DRIVER_DATA(pnd)->hCard),
                     (void *)&(DRIVER_DATA(pnd)->ioCard.dwProtocol)) !=
        SCARD_S_SUCCESS) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s",
              "PCSC connect failed");
      goto error;
    }
  }
  card_connected = true;

  DRIVER_DATA(pnd)->ioCard.cbPciLength = sizeof(SCARD_IO_REQUEST);

  char *pcFirmware = acr122_pcsc_firmware(pnd);
  if (!acr122_is_acr122u_firmware(pcFirmware))
    goto error;

  snprintf(pnd->name, sizeof(pnd->name), "%s / %s", ndd.pcsc_device_name,
           pcFirmware);
  CHIP_DATA(pnd)->timer_correction = 50;
  pnd->driver = &acr122_pcsc_driver;

  if (pn53x_init(pnd) < 0)
    goto error;

  free(ndd.pcsc_device_name);
  return pnd;

error:
  free(ndd.pcsc_device_name);
  if (card_connected)
    SCardDisconnect(DRIVER_DATA(pnd)->hCard, SCARD_LEAVE_CARD);
  if (context_acquired)
    acr122_pcsc_free_scardcontext();
  if (pnd && pnd->chip_data)
    pn53x_data_free(pnd);
  nfc_device_free(pnd);
  return NULL;
}

static void
acr122_pcsc_close(nfc_device *pnd)
{
  pn53x_idle(pnd);

  SCardDisconnect(DRIVER_DATA(pnd)->hCard, SCARD_LEAVE_CARD);
  acr122_pcsc_free_scardcontext();

  pn53x_data_free(pnd);
  nfc_device_free(pnd);
}

static int
acr122_pcsc_send(nfc_device *pnd, const uint8_t *pbtData, const size_t szData,
                 int timeout)
{
  (void)timeout;

  uint8_t abtTxBuf[ACR122_PCSC_WRAP_LEN + ACR122_PCSC_COMMAND_LEN];
  size_t tx_len = acr122_build_direct_transmit_apdu(abtTxBuf, sizeof(abtTxBuf),
                                                    pbtData, szData);
  if (tx_len == 0) {
    pnd->last_error = NFC_EINVARG;
    return pnd->last_error;
  }

  LOG_HEX(NFC_LOG_GROUP_COM, "TX", abtTxBuf, tx_len);

  DRIVER_DATA(pnd)->szRx = 0;
  DWORD dwRxLen = sizeof(DRIVER_DATA(pnd)->abtRx);

  if (DRIVER_DATA(pnd)->ioCard.dwProtocol == SCARD_PROTOCOL_UNDEFINED) {
    if (SCardControl(DRIVER_DATA(pnd)->hCard,
                     IOCTL_CCID_ESCAPE_SCARD_CTL_CODE, abtTxBuf, tx_len,
                     DRIVER_DATA(pnd)->abtRx, ACR122_PCSC_RESPONSE_LEN,
                     &dwRxLen) != SCARD_S_SUCCESS) {
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
  } else {
    if (SCardTransmit(DRIVER_DATA(pnd)->hCard, &(DRIVER_DATA(pnd)->ioCard),
                      abtTxBuf, tx_len, NULL, DRIVER_DATA(pnd)->abtRx,
                      &dwRxLen) != SCARD_S_SUCCESS) {
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
  }

  if (DRIVER_DATA(pnd)->ioCard.dwProtocol == SCARD_PROTOCOL_T0) {
    if (dwRxLen != 2) {
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
    if (DRIVER_DATA(pnd)->abtRx[0] == SCARD_OPERATION_ERROR) {
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
    if (DRIVER_DATA(pnd)->abtRx[0] != SCARD_OPERATION_SUCCESS) {
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
  } else {
    DRIVER_DATA(pnd)->szRx = dwRxLen;
  }

  return NFC_SUCCESS;
}

static int
acr122_pcsc_receive(nfc_device *pnd, uint8_t *pbtData, const size_t szData,
                    int timeout)
{
  (void)timeout;

  if (DRIVER_DATA(pnd)->ioCard.dwProtocol == SCARD_PROTOCOL_T0) {
    DWORD dwRxLen = sizeof(DRIVER_DATA(pnd)->abtRx);
    uint8_t abtRxCmd[5];
    size_t apdu_len =
        acr122_build_get_additional_data_apdu(abtRxCmd, sizeof(abtRxCmd),
                                              DRIVER_DATA(pnd)->abtRx[1]);
    if (apdu_len == 0) {
      pnd->last_error = NFC_EINVARG;
      return pnd->last_error;
    }

    if (SCardTransmit(DRIVER_DATA(pnd)->hCard, &(DRIVER_DATA(pnd)->ioCard),
                      abtRxCmd, apdu_len, NULL, DRIVER_DATA(pnd)->abtRx,
                      &dwRxLen) != SCARD_S_SUCCESS) {
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
    DRIVER_DATA(pnd)->szRx = dwRxLen;
  }

  LOG_HEX(NFC_LOG_GROUP_COM, "RX", DRIVER_DATA(pnd)->abtRx,
          DRIVER_DATA(pnd)->szRx);

  if (DRIVER_DATA(pnd)->szRx < 4 || (DRIVER_DATA(pnd)->szRx - 4) > szData) {
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  struct acr122_status_word status_word;
  if (!acr122_parse_status_words(
          DRIVER_DATA(pnd)->abtRx + DRIVER_DATA(pnd)->szRx - 2, 2,
          &status_word) ||
      !status_word.ok) {
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  const int len = (int)(DRIVER_DATA(pnd)->szRx - 4);
  if (nfc_safe_memcpy(pbtData, szData, DRIVER_DATA(pnd)->abtRx + 2, len) < 0) {
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  return len;
}

static char *
acr122_pcsc_firmware(nfc_device *pnd)
{
  static char abtFw[11];
  DWORD dwFwLen = sizeof(abtFw) - 1;

  if (nfc_secure_memset(abtFw, 0x00, sizeof(abtFw)) < 0)
    return abtFw;

  uint8_t abtGetFw[sizeof(struct acr122_apdu_header)];
  size_t apdu_len =
      acr122_build_get_firmware_version_apdu(abtGetFw, sizeof(abtGetFw));
  if (apdu_len == 0)
    return abtFw;

  uint32_t uiResult;
  if (DRIVER_DATA(pnd)->ioCard.dwProtocol == SCARD_PROTOCOL_UNDEFINED) {
    uiResult = SCardControl(DRIVER_DATA(pnd)->hCard,
                            IOCTL_CCID_ESCAPE_SCARD_CTL_CODE, abtGetFw,
                            apdu_len, (uint8_t *)abtFw, dwFwLen, &dwFwLen);
  } else {
    uiResult = SCardTransmit(DRIVER_DATA(pnd)->hCard,
                             &(DRIVER_DATA(pnd)->ioCard), abtGetFw, apdu_len,
                             NULL, (uint8_t *)abtFw, &dwFwLen);
  }

  if (uiResult != SCARD_S_SUCCESS) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "No ACR122 firmware received, Error: %08x", uiResult);
  }

  return abtFw;
}

const struct pn53x_io acr122_pcsc_io = {
    .send = acr122_pcsc_send,
    .receive = acr122_pcsc_receive,
};

const struct nfc_driver acr122_pcsc_driver = {
    .name = ACR122_PCSC_DRIVER_NAME,
    .scan = acr122_pcsc_scan,
    .open = acr122_pcsc_open,
    .close = acr122_pcsc_close,
    .strerror = pn53x_strerror,

    .initiator_init = pn53x_initiator_init,
    .initiator_init_secure_element = NULL,
    .initiator_select_passive_target = pn53x_initiator_select_passive_target,
    .initiator_poll_target = pn53x_initiator_poll_target,
    .initiator_select_dep_target = pn53x_initiator_select_dep_target,
    .initiator_deselect_target = pn53x_initiator_deselect_target,
    .initiator_transceive_bytes = pn53x_initiator_transceive_bytes,
    .initiator_transceive_bits = pn53x_initiator_transceive_bits,
    .initiator_transceive_bytes_timed = pn53x_initiator_transceive_bytes_timed,
    .initiator_transceive_bits_timed = pn53x_initiator_transceive_bits_timed,
    .initiator_target_is_present = pn53x_initiator_target_is_present,

    .target_init = pn53x_target_init,
    .target_send_bytes = pn53x_target_send_bytes,
    .target_receive_bytes = pn53x_target_receive_bytes,
    .target_send_bits = pn53x_target_send_bits,
    .target_receive_bits = pn53x_target_receive_bits,

    .device_set_property_bool = pn53x_set_property_bool,
    .device_set_property_int = pn53x_set_property_int,
    .get_supported_modulation = pn53x_get_supported_modulation,
    .get_supported_baud_rate = pn53x_get_supported_baud_rate,
    .device_get_information_about = pn53x_get_information_about,

    .abort_command = NULL,
    .idle = pn53x_idle,
    .powerdown = NULL,
};
