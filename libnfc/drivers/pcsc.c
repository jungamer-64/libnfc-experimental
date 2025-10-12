/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2019      Frank Morgner
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2020      Feitian Technologies
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
 * @file pcsc.c
 * @brief Driver for non-ACR122 devices behind PC/SC
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif // HAVE_CONFIG_H

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <time.h>
#include <limits.h>

#include <nfc/nfc.h>

#include "drivers/pcsc.h"
#include "nfc-internal.h"
#include "nfc-secure.h"

// Bus
#ifdef __APPLE__
#include <PCSC/winscard.h>
#include <PCSC/wintypes.h>
// define from pcsclite for apple
#define SCARD_AUTOALLOCATE (DWORD)(-1)

#define SCARD_ATTR_VALUE(Class, Tag) ((((ULONG)(Class)) << 16) | ((ULONG)(Tag)))

#define SCARD_CLASS_VENDOR_INFO 1    /**< Vendor information definitions */
#define SCARD_CLASS_COMMUNICATIONS 2 /**< Communication definitions */
#define SCARD_CLASS_PROTOCOL 3       /**< Protocol definitions */
#define SCARD_CLASS_POWER_MGMT 4     /**< Power Management definitions */
#define SCARD_CLASS_SECURITY 5       /**< Security Assurance definitions */
#define SCARD_CLASS_MECHANICAL 6     /**< Mechanical characteristic definitions */
#define SCARD_CLASS_VENDOR_DEFINED 7 /**< Vendor specific definitions */
#define SCARD_CLASS_IFD_PROTOCOL 8   /**< Interface Device Protocol options */
#define SCARD_CLASS_ICC_STATE 9      /**< ICC State specific definitions */
#define SCARD_CLASS_SYSTEM 0x7fff    /**< System-specific definitions */

#define SCARD_ATTR_VENDOR_NAME SCARD_ATTR_VALUE(SCARD_CLASS_VENDOR_INFO, 0x0100)          /**< Vendor name. */
#define SCARD_ATTR_VENDOR_IFD_TYPE SCARD_ATTR_VALUE(SCARD_CLASS_VENDOR_INFO, 0x0101)      /**< Vendor-supplied interface device type (model designation of reader). */
#define SCARD_ATTR_VENDOR_IFD_VERSION SCARD_ATTR_VALUE(SCARD_CLASS_VENDOR_INFO, 0x0102)   /**< Vendor-supplied interface device version (DWORD in the form 0xMMmmbbbb where MM = major version, mm = minor version, and bbbb = build number). */
#define SCARD_ATTR_VENDOR_IFD_SERIAL_NO SCARD_ATTR_VALUE(SCARD_CLASS_VENDOR_INFO, 0x0103) /**< Vendor-supplied interface device serial number. */
#define SCARD_ATTR_ICC_TYPE_PER_ATR SCARD_ATTR_VALUE(SCARD_CLASS_ICC_STATE, 0x0304)       /**< Single byte indicating smart card type */
#else
#ifndef _Win32
#include <reader.h>
#endif
#include <winscard.h>
#endif

#ifdef WIN32
#include <windows.h>
#endif

#define PCSC_DRIVER_NAME "pcsc"

#include <nfc/nfc.h>

#define LOG_GROUP NFC_LOG_GROUP_DRIVER
#define LOG_CATEGORY "libnfc.driver.pcsc"

static const char *supported_devices[] = {
    "ACS ACR122",       // ACR122U & Touchatag, last version
    "ACS ACR 38U-CCID", // Touchatag, early version
    "ACS ACR38U-CCID",  // Touchatag, early version, under MacOSX
    "ACS AET65",        // Touchatag using CCID driver version >= 1.4.6
    "    CCID USB",     // ??
    NULL};

struct pcsc_data
{
  SCARDHANDLE hCard;
  SCARD_IO_REQUEST ioCard;
  DWORD dwShareMode;
  DWORD last_error;
};

#define DRIVER_DATA(pnd) ((struct pcsc_data *)(pnd->driver_data))

static SCARDCONTEXT _SCardContext;
static int _iSCardContextRefCount = 0;

const nfc_baud_rate pcsc_supported_brs[] = {NBR_106, NBR_424, 0};
const nfc_modulation_type pcsc_supported_mts[] = {NMT_ISO14443A, NMT_ISO14443B, 0};

static SCARDCONTEXT *
pcsc_get_scardcontext(void)
{
  if (_iSCardContextRefCount == 0)
  {
    if (SCardEstablishContext(SCARD_SCOPE_USER, NULL, NULL, &_SCardContext) != SCARD_S_SUCCESS)
      return NULL;
  }
  _iSCardContextRefCount++;

  return &_SCardContext;
}

static void
pcsc_free_scardcontext(void)
{
  if (_iSCardContextRefCount)
  {
    _iSCardContextRefCount--;
    if (!_iSCardContextRefCount)
    {
      SCardReleaseContext(_SCardContext);
    }
  }
}

#define ICC_TYPE_UNKNOWN 0
#define ICC_TYPE_14443A 5
#define ICC_TYPE_14443B 6

bool is_pcsc_reader_vendor_feitian(const struct nfc_device *pnd);

static int pcsc_transmit(struct nfc_device *pnd, const uint8_t *tx, const size_t tx_len, uint8_t *rx, size_t *rx_len)
{
  struct pcsc_data *data = pnd->driver_data;
  DWORD dw_rx_len = *rx_len;
  // in libfreefare, tx_len = 1, and it leads to 0x80100008 error, with PC/SC reader, the input tx_len at least two bytes for the SW value
  // so if found the reader is Feitian reader, we set to 2
  if (is_pcsc_reader_vendor_feitian(pnd))
  {
    if (dw_rx_len == 1)
    {
      dw_rx_len = 2;
    }
    else
    {
      dw_rx_len += 2; // in libfreefare, some data length send not include sw1 and sw2, so add it.
    }
  }

  LOG_HEX(NFC_LOG_GROUP_COM, "TX", tx, tx_len);

  data->last_error = SCardTransmit(data->hCard, &data->ioCard, tx, tx_len,
                                   NULL, rx, &dw_rx_len);
  if (data->last_error != SCARD_S_SUCCESS)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s", "PCSC transmit failed");
    return NFC_EIO;
  }
  *rx_len = dw_rx_len;

  LOG_HEX(NFC_LOG_GROUP_COM, "RX", rx, *rx_len);

  return NFC_SUCCESS;
}

static int pcsc_get_status(struct nfc_device *pnd, int *target_present, uint8_t *atr, size_t *atr_len)
{
  struct pcsc_data *data = pnd->driver_data;
  DWORD dw_atr_len = *atr_len, reader_len, state, protocol;

  data->last_error = SCardStatus(data->hCard, NULL, &reader_len, &state, &protocol, atr, &dw_atr_len);
  if (data->last_error != SCARD_S_SUCCESS && data->last_error != SCARD_W_RESET_CARD)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Get status failed");
    return NFC_EIO;
  }

  *target_present = state & SCARD_PRESENT;
  *atr_len = dw_atr_len;

  return NFC_SUCCESS;
}

static int pcsc_reconnect(struct nfc_device *pnd, DWORD share_mode, DWORD protocol, DWORD disposition)
{
  struct pcsc_data *data = pnd->driver_data;

  data->last_error = SCardReconnect(data->hCard, share_mode, protocol, disposition, &data->ioCard.dwProtocol);
  if (data->last_error != SCARD_S_SUCCESS && data->last_error != SCARD_W_RESET_CARD)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Reconnect failed");
    return NFC_EIO;
  }

  data->dwShareMode = share_mode;

  return NFC_SUCCESS;
}

static uint8_t pcsc_get_icc_type(const struct nfc_device *pnd)
{
  struct pcsc_data *data = pnd->driver_data;
  uint8_t it = 0;
  DWORD dwItLen = sizeof it;
  data->last_error = SCardGetAttrib(data->hCard, SCARD_ATTR_ICC_TYPE_PER_ATR, &it, &dwItLen);
  return it;
}

static bool is_pcsc_reader_vendor(const struct nfc_device *pnd, const char *target_vendor_name)
{
  bool isTarget = false;
  /* Safely check device name length with bounds check (CWE-126) */
  if (pnd == NULL || nfc_safe_strlen(pnd->name, DEVICE_NAME_LENGTH) == 0)
  {
    return isTarget;
  }

  return isTarget = (strstr(pnd->name, target_vendor_name)) ? true : false;
}

bool is_pcsc_reader_vendor_feitian(const struct nfc_device *pnd)
{
  return is_pcsc_reader_vendor(pnd, "Feitian") || is_pcsc_reader_vendor(pnd, "FeiTian") || is_pcsc_reader_vendor(pnd, "feitian") || is_pcsc_reader_vendor(pnd, "FEITIAN");
}

static void pcsc_delay(unsigned long microseconds)
{
#ifdef WIN32
  Sleep((DWORD)((microseconds + 999UL) / 1000UL));
#else
  struct timespec request = {
      .tv_sec = (time_t)(microseconds / 1000000UL),
      .tv_nsec = (long)(microseconds % 1000000UL) * 1000L};

  while (nanosleep(&request, &request) == -1 && errno == EINTR)
  {
    // Retry with remaining time when interrupted
  }
#endif
}

// get atqa by send apdu
static int pcsc_get_atqa(struct nfc_device *pnd, uint8_t *atqa, size_t atqa_len)
{
  const uint8_t get_data[] = {0xFF, 0xCA, 0x03, 0x00, 0x00};
  uint8_t resp[256 + 2];
  size_t resp_len = sizeof resp;

  pnd->last_error = pcsc_transmit(pnd, get_data, sizeof get_data, resp, &resp_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (resp_len < 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Reader doesn't support request for ATQA");
    pnd->last_error = NFC_EDEVNOTSUPP;
    return pnd->last_error;
  }
  if (atqa_len < resp_len - 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "ATQA length is wrong");
    pnd->last_error = NFC_ESOFT;
    return pnd->last_error;
  }

  // Safe copy ATQA data (PC/SC response minus SW1SW2)
  size_t atqa_data_len = resp_len - 2;
  if (nfc_safe_memcpy(atqa, atqa_len, resp, atqa_data_len) < 0)
  {
    pnd->last_error = NFC_ECHIP;
    return pnd->last_error;
  }
  return atqa_data_len;
}

// get ats by send apdu
static int pcsc_get_ats(struct nfc_device *pnd, uint8_t *ats, size_t ats_len)
{
  const uint8_t get_data[] = {0xFF, 0xCA, 0x01, 0x00, 0x00};
  uint8_t resp[256 + 2];
  size_t resp_len = sizeof resp;

  pnd->last_error = pcsc_transmit(pnd, get_data, sizeof get_data, resp, &resp_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (resp_len <= 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Reader doesn't support request for ATS");
    pnd->last_error = NFC_EDEVNOTSUPP;
    return pnd->last_error;
  }
  if (ats_len < resp_len - 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "ATS length is wrong");
    pnd->last_error = NFC_ESOFT;
    return pnd->last_error;
  }

  // Safe copy ATS data (skip TL byte, minus SW1SW2)
  if (resp_len < 3)
  {
    pnd->last_error = NFC_ESOFT;
    return pnd->last_error;
  }
  size_t ats_data_len = resp_len - 3;
  if (nfc_safe_memcpy(ats, ats_len, resp + 1, ats_data_len) < 0)
  {
    pnd->last_error = NFC_ECHIP;
    return pnd->last_error;
  }
  return ats_data_len;
}

// get sak by send apdu
static int pcsc_get_sak(struct nfc_device *pnd, uint8_t *sak, size_t sak_len)
{
  const uint8_t get_data[] = {0xFF, 0xCA, 0x02, 0x00, 0x00};
  uint8_t resp[256 + 2];
  size_t resp_len = sizeof resp;

  pnd->last_error = pcsc_transmit(pnd, get_data, sizeof get_data, resp, &resp_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (resp_len < 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Reader doesn't support request for SAK");
    pnd->last_error = NFC_EDEVNOTSUPP;
    return pnd->last_error;
  }
  if (sak_len < resp_len - 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "SAK length is wrong");
    pnd->last_error = NFC_ESOFT;
    return pnd->last_error;
  }

  // Safe copy SAK data (PC/SC response minus SW1SW2)
  size_t sak_data_len = resp_len - 2;
  if (nfc_safe_memcpy(sak, sak_len, resp, sak_data_len) < 0)
  {
    pnd->last_error = NFC_ECHIP;
    return pnd->last_error;
  }
  return sak_data_len;
}

static int pcsc_get_uid(struct nfc_device *pnd, uint8_t *uid, size_t uid_len)
{
  const uint8_t get_data[] = {0xFF, 0xCA, 0x00, 0x00, 0x00};
  uint8_t resp[256 + 2];
  size_t resp_len = sizeof resp;

  pnd->last_error = pcsc_transmit(pnd, get_data, sizeof get_data, resp, &resp_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (resp_len < 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Reader doesn't support request for UID");
    pnd->last_error = NFC_EDEVNOTSUPP;
    return pnd->last_error;
  }
  if (uid_len < resp_len - 2)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "UID too big");
    pnd->last_error = NFC_ESOFT;
    return pnd->last_error;
  }

  // Safe copy UID data (PC/SC response minus SW1SW2)
  size_t uid_data_len = resp_len - 2;
  if (nfc_safe_memcpy(uid, uid_len, resp, uid_data_len) < 0)
  {
    pnd->last_error = NFC_ECHIP;
    return pnd->last_error;
  }
  return uid_data_len;
}

static bool icc_type_matches(uint8_t icc_type, uint8_t expected_type)
{
  return (icc_type == ICC_TYPE_UNKNOWN) || (icc_type == expected_type);
}

static bool iso14443a_uid_length_valid(int uid_length)
{
  return (uid_length <= 0) || uid_length == 4 || uid_length == 7 || uid_length == 10;
}

static bool iso14443a_atr_valid(const uint8_t *atr, size_t atr_length)
{
  if (!atr || atr_length < 5)
    return false;

  return atr[0] == 0x3B &&
         atr[1] == (0x80 | (uint8_t)(atr_length - 5)) &&
         atr[2] == 0x80 &&
         atr[3] == 0x01;
}

static int enrich_iso14443a_for_feitian(nfc_device *pnd, nfc_target *target)
{
  uint8_t atqa[2] = {0};
  int atqa_len = pcsc_get_atqa(pnd, atqa, sizeof(atqa));
  if (atqa_len >= 2)
  {
    if (nfc_safe_memcpy(target->nti.nai.abtAtqa, sizeof(target->nti.nai.abtAtqa), atqa, 2) < 0)
      return NFC_ECHIP;

    if (atqa[0] != 0x00 && atqa[0] != 0x03)
    {
      target->nti.nai.abtAtqa[0] = atqa[1];
      target->nti.nai.abtAtqa[1] = atqa[0];
    }
  }
  else if (atqa_len < 0 && atqa_len != NFC_EDEVNOTSUPP)
  {
    return atqa_len;
  }

  uint8_t sak[1] = {0};
  int sak_len = pcsc_get_sak(pnd, sak, sizeof(sak));
  if (sak_len >= 1)
  {
    target->nti.nai.btSak = sak[0];
  }
  else if (sak_len < 0 && sak_len != NFC_EDEVNOTSUPP)
  {
    return sak_len;
  }

  uint8_t ats[sizeof(target->nti.nai.abtAts)] = {0};
  int ats_len = pcsc_get_ats(pnd, ats, sizeof(ats));
  if (ats_len > 0)
  {
    if (nfc_safe_memcpy(target->nti.nai.abtAts, sizeof(target->nti.nai.abtAts), ats, ats_len) < 0)
      return NFC_ECHIP;
    target->nti.nai.szAtsLen = (size_t)ats_len;
  }
  else if (ats_len < 0 && ats_len != NFC_EDEVNOTSUPP)
  {
    return ats_len;
  }

  return NFC_SUCCESS;
}

static int fill_iso14443a_target(struct nfc_device *pnd, uint8_t icc_type, const uint8_t *atr, size_t atr_len, const uint8_t *uid, int uid_len, nfc_target *target)
{
  if (!target || !icc_type_matches(icc_type, ICC_TYPE_14443A) || !iso14443a_uid_length_valid(uid_len) || !iso14443a_atr_valid(atr, atr_len))
    return NFC_EINVARG;

  if (nfc_secure_memset(target, 0x00, sizeof(*target)) < 0)
    return NFC_ECHIP;

  target->nm.nmt = NMT_ISO14443A;
  target->nm.nbr = pcsc_supported_brs[0];

  if (uid_len > 0)
  {
    if (nfc_safe_memcpy(target->nti.nai.abtUid, sizeof(target->nti.nai.abtUid), uid, uid_len) < 0)
      return NFC_ECHIP;
    target->nti.nai.szUidLen = uid_len;
  }

  if (is_pcsc_reader_vendor_feitian(pnd))
    return enrich_iso14443a_for_feitian(pnd, target);

  target->nti.nai.btSak = 0x20;
  if (nfc_safe_memcpy(target->nti.nai.abtAts, sizeof(target->nti.nai.abtAts), "\x75\x77\x81\x02", 4) < 0)
    return NFC_ECHIP;

  size_t hist_len = atr_len - 5;
  if (nfc_safe_memcpy(target->nti.nai.abtAts + 4, sizeof(target->nti.nai.abtAts) - 4, atr + 4, hist_len) < 0)
    return NFC_ECHIP;
  target->nti.nai.szAtsLen = 4 + (uint8_t)hist_len;

  return NFC_SUCCESS;
}

static bool iso14443b_uid_length_valid(int uid_length)
{
  return (uid_length <= 0) || uid_length == 8;
}

static bool iso14443b_atr_valid(const uint8_t *atr, size_t atr_length)
{
  if (!atr)
    return false;

  return atr_length == (5 + 8) &&
         atr[0] == 0x3B &&
         atr[1] == (0x80 | 0x08) &&
         atr[2] == 0x80 &&
         atr[3] == 0x01;
}

static int fill_iso14443b_target(struct nfc_device *pnd, uint8_t icc_type, const uint8_t *atr, size_t atr_len, int uid_len, nfc_target *target)
{
  (void)pnd;
  if (!target || !icc_type_matches(icc_type, ICC_TYPE_14443B) || !iso14443b_uid_length_valid(uid_len) || !iso14443b_atr_valid(atr, atr_len))
    return NFC_EINVARG;

  if (nfc_secure_memset(target, 0x00, sizeof(*target)) < 0)
    return NFC_ECHIP;

  target->nm.nmt = NMT_ISO14443B;
  target->nm.nbr = pcsc_supported_brs[0];

  if (nfc_safe_memcpy(target->nti.nbi.abtApplicationData, sizeof(target->nti.nbi.abtApplicationData), atr + 4, 4) < 0)
    return NFC_ECHIP;
  if (nfc_safe_memcpy(target->nti.nbi.abtProtocolInfo, sizeof(target->nti.nbi.abtProtocolInfo), atr + 8, 3) < 0)
    return NFC_ECHIP;
  target->nti.nbi.abtProtocolInfo[1] = 0x01;

  return NFC_SUCCESS;
}

static int pcsc_props_to_target(struct nfc_device *pnd, uint8_t icc_type, const uint8_t *atr, size_t atr_len, const uint8_t *uid, int uid_len, const nfc_modulation_type modulation_type, nfc_target *target)
{
  if (!target)
    return NFC_EINVARG;

  switch (modulation_type)
  {
  case NMT_ISO14443A:
    return fill_iso14443a_target(pnd, icc_type, atr, atr_len, uid, uid_len, target);
  case NMT_ISO14443B:
    return fill_iso14443b_target(pnd, icc_type, atr, atr_len, uid_len, target);
  default:
    return NFC_EINVARG;
  }
}

#define PCSC_MAX_DEVICES 16

static bool pcsc_is_supported_reader(const char *reader_name)
{
  if (!reader_name)
    return false;

  for (int i = 0; supported_devices[i]; i++)
  {
    size_t prefix_len = nfc_safe_strlen(supported_devices[i], 256);
    if (strncmp(supported_devices[i], reader_name, prefix_len) == 0)
      return true;
  }

  return false;
}
/**
 * @brief List opened devices
 *
 * Probe PCSC to find any reader but the ACR122 devices (ACR122U and Touchatag/Tikitag).
 *
 * @param connstring array of nfc_connstring where found device's connection strings will be stored.
 * @param connstrings_len size of connstrings array.
 * @return number of devices found.
 */
static size_t
pcsc_scan(const nfc_context *context, nfc_connstring connstrings[], const size_t connstrings_len)
{
  (void)context;
  size_t szPos = 0;
  char acDeviceNames[256 + 64 * PCSC_MAX_DEVICES];
  size_t szDeviceNamesLen = sizeof(acDeviceNames);
  SCARDCONTEXT *pscc;

  // Clear the reader list (safe initialization)
  if (nfc_secure_memset(acDeviceNames, '\0', szDeviceNamesLen) < 0)
    return 0;

  // Test if context succeeded
  if (!(pscc = pcsc_get_scardcontext()))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "Warning: %s", "PCSC context not found (make sure PCSC daemon is running).");
    return 0;
  }
  // Retrieve the string array of all available pcsc readers
  DWORD dwDeviceNamesLen = szDeviceNamesLen;
  if (SCardListReaders(*pscc, NULL, acDeviceNames, &dwDeviceNamesLen) != SCARD_S_SUCCESS)
    return 0;

  size_t device_found = 0;
  while ((acDeviceNames[szPos] != '\0') && (device_found < connstrings_len))
  {
    bool is_supported = pcsc_is_supported_reader(acDeviceNames + szPos);

    if (is_supported)
    {
      // Supported non-ACR122 device found
      snprintf(connstrings[device_found], sizeof(nfc_connstring), "%s:%s", PCSC_DRIVER_NAME, acDeviceNames + szPos);
      device_found++;
    }
    else
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Skipping PCSC device [%s] as it is supported by acr122_pcsc driver.", acDeviceNames + szPos);
    }

    // Find next device name position
    while (acDeviceNames[szPos++] != '\0')
      ;
  }
  pcsc_free_scardcontext();

  return device_found;
}

struct pcsc_descriptor
{
  char *pcsc_device_name;
};

static void pcsc_descriptor_cleanup(struct pcsc_descriptor *descriptor)
{
  if (descriptor && descriptor->pcsc_device_name)
  {
    free(descriptor->pcsc_device_name);
    descriptor->pcsc_device_name = NULL;
  }
}

static bool copy_connstring_value(const nfc_connstring source, nfc_connstring destination)
{
  size_t length = nfc_safe_strlen(source, sizeof(nfc_connstring));
  if (length >= sizeof(nfc_connstring))
    return false;

  if (nfc_safe_memcpy(destination, sizeof(nfc_connstring), source, length) < 0)
    return false;

  destination[length] = '\0';
  return true;
}

static bool parse_device_index(const char *value, size_t *index)
{
  if (!value || !index)
    return false;

  size_t length = nfc_safe_strlen(value, 6);
  if (length == 0 || length > 4)
    return false;

  for (size_t i = 0; i < length; i++)
  {
    if (!isdigit((unsigned char)value[i]))
      return false;
  }

  errno = 0;
  unsigned long parsed = strtoul(value, NULL, 10);
  if (errno != 0 || parsed > SIZE_MAX)
    return false;

  *index = (size_t)parsed;
  return true;
}

static bool resolve_connstring_from_index(const nfc_context *context, size_t index, nfc_connstring resolved, struct pcsc_descriptor *descriptor)
{
  bool success = false;
  nfc_connstring *list = malloc(sizeof(nfc_connstring) * (index + 1));
  if (!list)
  {
    perror("malloc");
    return false;
  }

  size_t found = pcsc_scan(context, list, index + 1);
  if (found > index && copy_connstring_value(list[index], resolved))
  {
    pcsc_descriptor_cleanup(descriptor);
    int decode_level = connstring_decode(resolved, PCSC_DRIVER_NAME, "pcsc", &descriptor->pcsc_device_name, NULL);
    success = decode_level >= 2;
    if (!success)
      pcsc_descriptor_cleanup(descriptor);
  }

  free(list);
  return success;
}

static bool resolve_pcsc_connection(const nfc_context *context, const nfc_connstring connstring, struct pcsc_descriptor *descriptor, nfc_connstring resolved)
{
  int decode_level = connstring_decode(connstring, PCSC_DRIVER_NAME, "pcsc", &descriptor->pcsc_device_name, NULL);
  if (decode_level < 1)
    return false;

  if (decode_level == 1)
  {
    nfc_connstring discovered;
    if (pcsc_scan(context, &discovered, 1) < 1)
      return false;

    if (!copy_connstring_value(discovered, resolved))
      return false;

    pcsc_descriptor_cleanup(descriptor);
    decode_level = connstring_decode(resolved, PCSC_DRIVER_NAME, "pcsc", &descriptor->pcsc_device_name, NULL);
    return decode_level >= 2;
  }

  if (!copy_connstring_value(connstring, resolved))
    return false;

  size_t index = 0;
  size_t name_length = nfc_safe_strlen(descriptor->pcsc_device_name, 256);
  if (name_length > 0 && name_length < 5 && parse_device_index(descriptor->pcsc_device_name, &index))
    return resolve_connstring_from_index(context, index, resolved, descriptor);

  return true;
}

static nfc_device *
pcsc_open(const nfc_context *context, const nfc_connstring connstring)
{
  struct pcsc_descriptor descriptor = {0};
  nfc_connstring resolved_connstring;
  nfc_device *pnd = NULL;

  if (!resolve_pcsc_connection(context, connstring, &descriptor, resolved_connstring))
    goto error;

  pnd = nfc_device_new(context, resolved_connstring);
  if (!pnd)
  {
    perror("malloc");
    goto error;
  }

  pnd->driver_data = malloc(sizeof(struct pcsc_data));
  if (!pnd->driver_data)
  {
    perror("malloc");
    goto error;
  }

  SCARDCONTEXT *pscc;

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Attempt to open %s", descriptor.pcsc_device_name);
  if (!(pscc = pcsc_get_scardcontext()))
    goto error;

  DRIVER_DATA(pnd)->last_error = SCardConnect(*pscc, descriptor.pcsc_device_name, SCARD_SHARE_DIRECT, 0 | 1, &(DRIVER_DATA(pnd)->hCard), (void *)&(DRIVER_DATA(pnd)->ioCard.dwProtocol));
  if (DRIVER_DATA(pnd)->last_error != SCARD_S_SUCCESS)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s", "PCSC connect failed");
    goto error;
  }

  DRIVER_DATA(pnd)->ioCard.cbPciLength = sizeof(SCARD_IO_REQUEST);
  DRIVER_DATA(pnd)->dwShareMode = SCARD_SHARE_DIRECT;

  snprintf(pnd->name, sizeof(pnd->name), "%s", descriptor.pcsc_device_name);
  pnd->driver = &pcsc_driver;

  pcsc_descriptor_cleanup(&descriptor);
  return pnd;

error:
  pcsc_descriptor_cleanup(&descriptor);
  nfc_device_free(pnd);
  return NULL;
}

static void
pcsc_close(nfc_device *pnd)
{
  SCardDisconnect(DRIVER_DATA(pnd)->hCard, SCARD_LEAVE_CARD);
  pcsc_free_scardcontext();

  nfc_device_free(pnd);
}

struct pcsc_error_entry
{
  LONG code;
  const char *message;
};

static const struct pcsc_error_entry pcsc_error_table[] = {
    {SCARD_S_SUCCESS, "Command successful."},
    {SCARD_F_INTERNAL_ERROR, "Internal error."},
    {SCARD_E_CANCELLED, "Command cancelled."},
    {SCARD_E_INVALID_HANDLE, "Invalid handle."},
    {SCARD_E_INVALID_PARAMETER, "Invalid parameter given."},
    {SCARD_E_INVALID_TARGET, "Invalid target given."},
    {SCARD_E_NO_MEMORY, "Not enough memory."},
    {SCARD_F_WAITED_TOO_LONG, "Waited too long."},
    {SCARD_E_INSUFFICIENT_BUFFER, "Insufficient buffer."},
    {SCARD_E_UNKNOWN_READER, "Unknown reader specified."},
    {SCARD_E_TIMEOUT, "Command timeout."},
    {SCARD_E_SHARING_VIOLATION, "Sharing violation."},
    {SCARD_E_NO_SMARTCARD, "No smart card inserted."},
    {SCARD_E_UNKNOWN_CARD, "Unknown card."},
    {SCARD_E_CANT_DISPOSE, "Cannot dispose handle."},
    {SCARD_E_PROTO_MISMATCH, "Card protocol mismatch."},
    {SCARD_E_NOT_READY, "Subsystem not ready."},
    {SCARD_E_INVALID_VALUE, "Invalid value given."},
    {SCARD_E_SYSTEM_CANCELLED, "System cancelled."},
    {SCARD_F_COMM_ERROR, "RPC transport error."},
    {SCARD_F_UNKNOWN_ERROR, "Unknown error."},
    {SCARD_E_INVALID_ATR, "Invalid ATR."},
    {SCARD_E_NOT_TRANSACTED, "Transaction failed."},
    {SCARD_E_READER_UNAVAILABLE, "Reader is unavailable."},
    {SCARD_E_PCI_TOO_SMALL, "PCI struct too small."},
    {SCARD_E_READER_UNSUPPORTED, "Reader is unsupported."},
    {SCARD_E_DUPLICATE_READER, "Reader already exists."},
    {SCARD_E_CARD_UNSUPPORTED, "Card is unsupported."},
    {SCARD_E_NO_SERVICE, "Service not available."},
    {SCARD_E_SERVICE_STOPPED, "Service was stopped."},
    {SCARD_E_NO_READERS_AVAILABLE, "Cannot find a smart card reader."},
    {SCARD_W_UNSUPPORTED_CARD, "Card is not supported."},
    {SCARD_W_UNRESPONSIVE_CARD, "Card is unresponsive."},
    {SCARD_W_UNPOWERED_CARD, "Card is unpowered."},
    {SCARD_W_RESET_CARD, "Card was reset."},
    {SCARD_W_REMOVED_CARD, "Card was removed."},
    {SCARD_E_UNSUPPORTED_FEATURE, "Feature not supported."}};

static const char *stringify_error(const LONG pcscError)
{
  for (size_t i = 0; i < sizeof(pcsc_error_table) / sizeof(pcsc_error_table[0]); i++)
  {
    if (pcsc_error_table[i].code == pcscError)
      return pcsc_error_table[i].message;
  }

  static char fallback[75];
  (void)snprintf(fallback, sizeof(fallback), "Unknown error: 0x%08lX", (unsigned long)pcscError);
  return fallback;
}

static const char *
pcsc_strerror(const struct nfc_device *pnd)
{
  return stringify_error(DRIVER_DATA(pnd)->last_error);
}

static int pcsc_initiator_init(struct nfc_device *pnd)
{
  (void)pnd;
  return NFC_SUCCESS;
}

static int pcsc_initiator_select_passive_target(struct nfc_device *pnd, const nfc_modulation nm, const uint8_t *pbtInitData, const size_t szInitData, nfc_target *pnt)
{
  uint8_t atr[MAX_ATR_SIZE];
  uint8_t uid[10];
  int target_present;
  size_t atr_len = sizeof atr;

  (void)pbtInitData;
  (void)szInitData;

  if (nm.nbr != pcsc_supported_brs[0] && nm.nbr != pcsc_supported_brs[1])
    return NFC_EINVARG;

  pnd->last_error = pcsc_get_status(pnd, &target_present, atr, &atr_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (!target_present)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "No target present");
    return NFC_ENOTSUCHDEV;
  }

  uint8_t icc_type = pcsc_get_icc_type(pnd);
  int uid_len = pcsc_get_uid(pnd, uid, sizeof uid);
  if (pcsc_props_to_target(pnd, icc_type, atr, atr_len, uid, uid_len, nm.nmt, pnt) != NFC_SUCCESS)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Type of target not supported");
    return NFC_EDEVNOTSUPP;
  }

  pnd->last_error = pcsc_reconnect(pnd, SCARD_SHARE_SHARED, SCARD_PROTOCOL_T0 | SCARD_PROTOCOL_T1, SCARD_LEAVE_CARD);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  return 1;
}

#if 0
static int pcsc_initiator_deselect_target(struct nfc_device *pnd)
{
  pnd->last_error = pcsc_reconnect(pnd, SCARD_SHARE_DIRECT, 0, SCARD_LEAVE_CARD);
  return pnd->last_error;
}
#endif

typedef int (*pcsc_feitian_handler)(struct nfc_device *, uint8_t, const uint8_t *, size_t, uint8_t *, size_t, size_t *);

static int feitian_execute_apdu(struct nfc_device *pnd, const uint8_t *apdu, size_t apdu_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  uint8_t response[256 + 2];
  size_t local_len = sizeof(response);

  LOG_HEX(NFC_LOG_GROUP_COM, "feitian reader pcsc apdu send:", apdu, apdu_len);
  pnd->last_error = pcsc_transmit(pnd, apdu, apdu_len, response, &local_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  LOG_HEX(NFC_LOG_GROUP_COM, "feitian reader pcsc apdu received:", response, local_len);

  if (local_len > rx_len)
    return NFC_ECHIP;

  if (nfc_safe_memcpy(rx, rx_len, response, local_len) < 0)
    return NFC_ECHIP;

  *resp_len = local_len;
  return NFC_SUCCESS;
}

static int feitian_handle_read(struct nfc_device *pnd, uint8_t command, const uint8_t *tx, size_t tx_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  (void)command;
  if (tx_len < 2)
    return NFC_EINVARG;

  uint8_t apdu[] = {0xFF, 0xB0, 0x00, tx[1], 0x10};
  return feitian_execute_apdu(pnd, apdu, sizeof(apdu), rx, rx_len, resp_len);
}

static int feitian_handle_write(struct nfc_device *pnd, uint8_t command, const uint8_t *tx, size_t tx_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  (void)command;
  if (tx_len < 2)
    return NFC_EINVARG;

  size_t data_len = tx_len - 2;
  uint8_t apdu[256] = {0};
  apdu[0] = 0xFF;
  apdu[1] = 0xD6;
  apdu[2] = 0x00;
  apdu[3] = tx[1];
  apdu[4] = (uint8_t)data_len;

  if (data_len > sizeof(apdu) - 5)
    return NFC_ECHIP;

  if (data_len > 0 && nfc_safe_memcpy(apdu + 5, sizeof(apdu) - 5, tx + 2, data_len) < 0)
    return NFC_ECHIP;

  return feitian_execute_apdu(pnd, apdu, 5 + data_len, rx, rx_len, resp_len);
}

static int feitian_handle_auth(struct nfc_device *pnd, uint8_t command, const uint8_t *tx, size_t tx_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  if (tx_len < 8)
    return NFC_EINVARG;

  uint8_t apdu[256] = {0};
  uint8_t discard[256 + 2];
  size_t discard_len = sizeof(discard);

  apdu[0] = 0xFF;
  apdu[1] = 0x82;
  apdu[2] = 0x00;
  apdu[3] = 0x01;
  apdu[4] = 0x06;

  if (nfc_safe_memcpy(apdu + 5, sizeof(apdu) - 5, tx + 2, 6) < 0)
    return NFC_ECHIP;

  pnd->last_error = pcsc_transmit(pnd, apdu, 11, discard, &discard_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (nfc_secure_memset(apdu, 0x00, sizeof(apdu)) < 0)
    return NFC_ECHIP;
  if (nfc_secure_memset(discard, 0x00, sizeof(discard)) < 0)
    return NFC_ECHIP;

  pcsc_delay(500000UL);

  apdu[0] = 0xFF;
  apdu[1] = 0x86;
  apdu[2] = 0x00;
  apdu[3] = 0x00;
  apdu[4] = 0x05;
  apdu[5] = 0x01;
  apdu[6] = 0x00;
  apdu[7] = tx[1];
  apdu[8] = command;
  apdu[9] = 0x01;

  return feitian_execute_apdu(pnd, apdu, 10, rx, rx_len, resp_len);
}

static int feitian_handle_value_operation(struct nfc_device *pnd, uint8_t command, const uint8_t *tx, size_t tx_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  if (tx_len < 2)
    return NFC_EINVARG;

  size_t payload_len = tx_len - 2;
  uint8_t apdu[256] = {0};
  apdu[0] = 0xFF;
  apdu[1] = (command == 0xC2) ? 0xD8 : 0xD7;
  apdu[2] = 0x00;
  apdu[3] = tx[1];
  apdu[4] = (command == 0xC2) ? (uint8_t)payload_len : 0x05;

  if (payload_len > sizeof(apdu) - 5)
    return NFC_ECHIP;

  if (payload_len > 0 && nfc_safe_memcpy(apdu + 5, sizeof(apdu) - 5, tx + 2, payload_len) < 0)
    return NFC_ECHIP;

  return feitian_execute_apdu(pnd, apdu, 5 + payload_len, rx, rx_len, resp_len);
}

static int feitian_handle_generic(struct nfc_device *pnd, uint8_t command, const uint8_t *tx, size_t tx_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  (void)command;
  if (tx_len > 256)
    return NFC_ECHIP;

  uint8_t apdu[256];
  if (nfc_safe_memcpy(apdu, sizeof(apdu), tx, tx_len) < 0)
    return NFC_ECHIP;

  return feitian_execute_apdu(pnd, apdu, tx_len, rx, rx_len, resp_len);
}

struct feitian_handler_entry
{
  uint8_t opcode;
  pcsc_feitian_handler handler;
};

static int feitian_route_command(struct nfc_device *pnd, const uint8_t *tx, size_t tx_len, uint8_t *rx, size_t rx_len, size_t *resp_len)
{
  if (tx_len == 0)
    return NFC_EINVARG;

  static const struct feitian_handler_entry handlers[] = {
      {0x30, feitian_handle_read},
      {0xA0, feitian_handle_write},
      {0xA2, feitian_handle_write},
      {0x60, feitian_handle_auth},
      {0x61, feitian_handle_auth},
      {0x1A, feitian_handle_auth},
      {0xC0, feitian_handle_value_operation},
      {0xC1, feitian_handle_value_operation},
      {0xC2, feitian_handle_value_operation}};

  uint8_t command = tx[0];
  for (size_t i = 0; i < sizeof(handlers) / sizeof(handlers[0]); i++)
  {
    if (handlers[i].opcode == command)
      return handlers[i].handler(pnd, command, tx, tx_len, rx, rx_len, resp_len);
  }

  return feitian_handle_generic(pnd, command, tx, tx_len, rx, rx_len, resp_len);
}

static int pcsc_initiator_transceive_bytes(struct nfc_device *pnd, const uint8_t *pbtTx, const size_t szTx, uint8_t *pbtRx, const size_t szRx, int timeout)
{
  size_t resp_len = szRx;

  // Timeout parameter is not used by this PC/SC implementation
  // PC/SC handles timeouts internally based on SCARD_SHARE_* mode
  (void)timeout;

  if (is_pcsc_reader_vendor_feitian(pnd))
  {
    LOG_HEX(NFC_LOG_GROUP_COM, "not feitian reader pcsc apdu send", pbtTx, szTx);

    int status = feitian_route_command(pnd, pbtTx, szTx, pbtRx, szRx, &resp_len);
    if (status != NFC_SUCCESS)
    {
      if (status < 0)
        pnd->last_error = status;
      return status;
    }
  }
  else
  {
    pnd->last_error = pcsc_transmit(pnd, pbtTx, szTx, pbtRx, &resp_len);
  }
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  return (int)resp_len;
}

static int pcsc_initiator_target_is_present(struct nfc_device *pnd, const nfc_target *pnt)
{
  uint8_t atr[MAX_ATR_SIZE];
  int target_present;
  size_t atr_len = sizeof atr;
  nfc_target nt;

  pnd->last_error = pcsc_get_status(pnd, &target_present, atr, &atr_len);
  if (pnd->last_error != NFC_SUCCESS)
    return pnd->last_error;

  if (!target_present)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "No target present");
    return NFC_ENOTSUCHDEV;
  }

  if (pnt)
  {
    if (pcsc_props_to_target(pnd, ICC_TYPE_UNKNOWN, atr, atr_len, NULL, 0, pnt->nm.nmt, &nt) != NFC_SUCCESS || pnt->nm.nmt != nt.nm.nmt || pnt->nm.nbr != nt.nm.nbr)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Target doesn't meet requirements");
      return NFC_ENOTSUCHDEV;
    }
  }
  return NFC_SUCCESS;
}

static int pcsc_device_set_property_bool(struct nfc_device *pnd, const nfc_property property, const bool enable)
{
  bool is_feitian = is_pcsc_reader_vendor_feitian(pnd);

  switch (property)
  {
  case NP_INFINITE_SELECT:
    return NFC_SUCCESS;
  case NP_AUTO_ISO14443_4:
  case NP_EASY_FRAMING:
    return (enable || is_feitian) ? NFC_SUCCESS : NFC_EDEVNOTSUPP;
  case NP_FORCE_ISO14443_A:
  case NP_HANDLE_CRC:
  case NP_HANDLE_PARITY:
  case NP_FORCE_SPEED_106:
    return enable ? NFC_SUCCESS : NFC_EDEVNOTSUPP;
  case NP_ACCEPT_INVALID_FRAMES:
  case NP_ACCEPT_MULTIPLE_FRAMES:
    return enable ? NFC_EDEVNOTSUPP : NFC_SUCCESS;
  case NP_ACTIVATE_FIELD:
    if (!enable)
    {
      struct pcsc_data *data = pnd->driver_data;
      pcsc_reconnect(pnd, data->dwShareMode, data->ioCard.dwProtocol, SCARD_RESET_CARD);
    }
    return NFC_SUCCESS;
  default:
    return NFC_EDEVNOTSUPP;
  }
}

static int pcsc_get_supported_modulation(struct nfc_device *pnd, const nfc_mode mode, const nfc_modulation_type **const supported_mt)
{
  (void)pnd;
  if (mode == N_TARGET || NULL == supported_mt)
    return NFC_EINVARG;
  *supported_mt = pcsc_supported_mts;
  return NFC_SUCCESS;
}

static int pcsc_get_supported_baud_rate(struct nfc_device *pnd, const nfc_mode mode, const nfc_modulation_type nmt, const nfc_baud_rate **const supported_br)
{
  (void)pnd;
  (void)nmt;
  if (mode == N_TARGET || NULL == supported_br)
    return NFC_EINVARG;
  *supported_br = pcsc_supported_brs;
  return NFC_SUCCESS;
}

static char *pcsc_duplicate_attribute(LPBYTE value, DWORD length)
{
  if (!value || length == 0)
    return NULL;

  size_t usable_len = strnlen((const char *)value, length);
  if (usable_len == 0)
    return NULL;

  char *copy = malloc(usable_len + 1);
  if (!copy)
    return NULL;

  if (nfc_safe_memcpy(copy, usable_len + 1, value, usable_len) < 0)
  {
    free(copy);
    return NULL;
  }

  copy[usable_len] = '\0';
  return copy;
}

static int
pcsc_get_information_about(nfc_device *pnd, char **pbuf)
{
  struct pcsc_data *data = pnd->driver_data;
  LPBYTE name = NULL, version = NULL, type = NULL, serial = NULL;
#ifdef __APPLE__
  DWORD name_len = 0, version_len = 0,
        type_len = 0, serial_len = 0;
#else
  DWORD name_len = SCARD_AUTOALLOCATE, version_len = SCARD_AUTOALLOCATE,
        type_len = SCARD_AUTOALLOCATE, serial_len = SCARD_AUTOALLOCATE;
#endif
  int res = NFC_SUCCESS;
  SCARDCONTEXT *pscc;

  if (!(pscc = pcsc_get_scardcontext()))
  {
    pnd->last_error = NFC_ESOFT;
    return pnd->last_error;
  }

  SCardGetAttrib(data->hCard, SCARD_ATTR_VENDOR_NAME, (LPBYTE)&name, &name_len);
  SCardGetAttrib(data->hCard, SCARD_ATTR_VENDOR_IFD_TYPE, (LPBYTE)&type, &type_len);
  SCardGetAttrib(data->hCard, SCARD_ATTR_VENDOR_IFD_VERSION, (LPBYTE)&version, &version_len);
  SCardGetAttrib(data->hCard, SCARD_ATTR_VENDOR_IFD_SERIAL_NO, (LPBYTE)&serial, &serial_len);

  char *model = pcsc_duplicate_attribute(name, name_len);
  char *version_str = pcsc_duplicate_attribute(version, version_len);
  char *vendor = pcsc_duplicate_attribute(type, type_len);
  char *serial_str = pcsc_duplicate_attribute(serial, serial_len);

  const char *model_text = model ? model : "unknown model";
  const char *version_prefix = version_str ? " " : "";
  const char *version_text = version_str ? version_str : "";
  const char *vendor_text = vendor ? vendor : "unknown vendor";
  const char *serial_prefix = serial_str ? "\nserial: " : "";
  const char *serial_text = serial_str ? serial_str : "";

  size_t buffer_len = strlen(model_text) + strlen(version_prefix) + strlen(version_text) +
                      3 + strlen(vendor_text) + strlen(serial_prefix) + strlen(serial_text) + 1 + 1;

  *pbuf = malloc(buffer_len);
  if (!*pbuf)
  {
    res = NFC_ESOFT;
    goto cleanup_strings;
  }

  int written = snprintf((char *)*pbuf, buffer_len, "%s%s%s (%s)%s%s\n", model_text, version_prefix, version_text, vendor_text, serial_prefix, serial_text);
  if (written < 0 || (size_t)written >= buffer_len)
  {
    free(*pbuf);
    *pbuf = NULL;
    res = NFC_ESOFT;
    goto cleanup_strings;
  }

cleanup_strings:
  free(model);
  free(version_str);
  free(vendor);
  free(serial_str);

error:
#ifdef __APPLE__
  if (pscc != NULL)
  {
    SCardReleaseContext(*pscc);
  }
  if (name != NULL)
  {
    free(name);
    name = NULL;
  }
  if (type != NULL)
  {
    free(type);
    type = NULL;
  }
  if (version != NULL)
  {
    free(version);
    version = NULL;
  }
  if (serial != NULL)
  {
    free(serial);
    serial = NULL;
  }
#else
  SCardFreeMemory(*pscc, name);
  SCardFreeMemory(*pscc, type);
  SCardFreeMemory(*pscc, version);
  SCardFreeMemory(*pscc, serial);
#endif

  pnd->last_error = res;
  return pnd->last_error;
}

const struct nfc_driver pcsc_driver = {
    .name = PCSC_DRIVER_NAME,
    .scan = pcsc_scan,
    .open = pcsc_open,
    .close = pcsc_close,
    .strerror = pcsc_strerror,

    .initiator_init = pcsc_initiator_init,
    .initiator_init_secure_element = NULL, // No secure-element support
    .initiator_select_passive_target = pcsc_initiator_select_passive_target,
    .initiator_poll_target = NULL,
    .initiator_select_dep_target = NULL,
    .initiator_deselect_target = NULL,
    .initiator_transceive_bytes = pcsc_initiator_transceive_bytes,
    .initiator_transceive_bits = NULL,
    .initiator_transceive_bytes_timed = NULL,
    .initiator_transceive_bits_timed = NULL,
    .initiator_target_is_present = pcsc_initiator_target_is_present,

    .target_init = NULL,
    .target_send_bytes = NULL,
    .target_receive_bytes = NULL,
    .target_send_bits = NULL,
    .target_receive_bits = NULL,

    .device_set_property_bool = pcsc_device_set_property_bool,
    .device_set_property_int = NULL,
    .get_supported_modulation = pcsc_get_supported_modulation,
    .get_supported_baud_rate = pcsc_get_supported_baud_rate,
    .device_get_information_about = pcsc_get_information_about,

    .abort_command = NULL, // Abort is not supported in this driver
    .idle = NULL,
    .powerdown = NULL,
};
