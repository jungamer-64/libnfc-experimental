/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2026      OpenAI
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
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
 * @file acr122_usb.c
 * @brief Driver for ACR122 using direct USB (without PCSC)
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif // HAVE_CONFIG_H

#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/select.h>
#ifdef _MSC_VER
#include <sys/types.h>
#endif

#include <nfc/nfc.h>

#include "buses/usbbus.h"
#include "chips/pn53x.h"
#include "chips/pn53x-internal.h"
#include "drivers/acr122-core.h"
#include "drivers/acr122_usb.h"
#include "nfc-common.h"
#include "nfc-internal.h"
#include "nfc-secure.h"

#define ACR122_USB_DRIVER_NAME "acr122_usb"

#define LOG_GROUP NFC_LOG_GROUP_DRIVER
#define LOG_CATEGORY "libnfc.driver.acr122_usb"

#define USB_INFINITE_TIMEOUT 0

#define DRIVER_DATA(pnd) ((struct acr122_usb_data *)(pnd->driver_data))

#pragma pack(push, 1)
struct acr122_usb_tama_frame {
  struct acr122_ccid_header ccid_header;
  struct acr122_apdu_header apdu_header;
  uint8_t tama_header;
  uint8_t tama_payload[254];
};

struct acr122_usb_apdu_frame {
  struct acr122_ccid_header ccid_header;
  struct acr122_apdu_header apdu_header;
  uint8_t apdu_payload[255];
};
#pragma pack(pop)

struct acr122_usb_data {
  usb_dev_handle *pudh;
  uint32_t uiEndPointIn;
  uint32_t uiEndPointOut;
  uint32_t uiMaxPacketSize;
  int interface_number;
  int configuration_value;
  int alternate_setting;
  volatile bool abort_flag;
  struct acr122_usb_tama_frame tama_frame;
  struct acr122_usb_apdu_frame apdu_frame;
};

const struct pn53x_io acr122_usb_io;

static int acr122_usb_init(nfc_device *pnd);
static int acr122_usb_ack(nfc_device *pnd);
static int acr122_usb_send_apdu(nfc_device *pnd,
                                uint8_t ins, uint8_t p1, uint8_t p2,
                                const uint8_t *data, size_t data_len,
                                uint8_t le, uint8_t *out, size_t out_size);

static void
acr122_usb_prepare_ccid_header(struct acr122_ccid_header *header,
                               uint8_t message_type)
{
  header->bMessageType = message_type;
  header->dwLength = 0;
  header->bSlot = 0x00;
  header->bSeq = 0x00;
  header->bMessageSpecific[0] = 0x00;
  header->bMessageSpecific[1] = 0x00;
  header->bMessageSpecific[2] = 0x00;
}

static int
acr122_usb_bulk_read(struct acr122_usb_data *data, uint8_t abtRx[],
                     const size_t szRx, const int timeout)
{
  int res =
      usb_bulk_read(data->pudh, data->uiEndPointIn, abtRx, szRx, timeout);
  if (res > 0) {
    LOG_HEX(NFC_LOG_GROUP_COM, "RX", abtRx, res);
  } else if (res < 0) {
    if (!usb_error_is_timeout(res)) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Unable to read from USB (%s)", _usb_strerror(res));
      res = NFC_EIO;
    } else {
      res = NFC_ETIMEOUT;
    }
  }
  return res;
}

static int
acr122_usb_bulk_write(struct acr122_usb_data *data, uint8_t abtTx[],
                      const size_t szTx, const int timeout)
{
  LOG_HEX(NFC_LOG_GROUP_COM, "TX", abtTx, szTx);
  int res = usb_bulk_write(data->pudh, data->uiEndPointOut, abtTx,
                           szTx, timeout);
  if (res > 0) {
    if (data->uiMaxPacketSize != 0 && (res % data->uiMaxPacketSize) == 0)
      usb_bulk_write(data->pudh, data->uiEndPointOut, NULL, 0, timeout);
  } else if (res < 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Unable to write to USB (%s)", _usb_strerror(res));
    if (usb_error_is_timeout(res))
      res = NFC_ETIMEOUT;
    else
      res = NFC_EIO;
  }
  return res;
}

static bool
acr122_usb_get_end_points(const struct usb_device *dev,
                          struct acr122_usb_data *data)
{
  struct usb_bulk_endpoints endpoints;
  if (!usb_device_get_bulk_endpoints(dev, &endpoints))
    return false;

  data->uiEndPointIn = endpoints.endpoint_in;
  data->uiEndPointOut = endpoints.endpoint_out;
  data->uiMaxPacketSize = endpoints.max_packet_size;
  data->interface_number = endpoints.interface_number;
  data->alternate_setting = endpoints.alternate_setting;
  return true;
}

static size_t
acr122_usb_scan(const nfc_context *context, nfc_connstring connstrings[],
                const size_t connstrings_len)
{
  (void)context;

  struct usb_device_list devices;
  if (usb_get_device_list(&devices) < 0)
    return 0;

  size_t device_found = 0;
  for (size_t i = 0; i < devices.count; i++) {
    const struct usb_device *dev = &devices.devices[i];
    const char *device_name =
        acr122_usb_device_name(dev->vendor_id, dev->product_id);
    if (device_name == NULL)
      continue;

    struct usb_bulk_endpoints endpoints;
    if (!usb_device_get_bulk_endpoints(dev, &endpoints))
      continue;

    usb_dev_handle *udev = NULL;
    if (usb_open(dev, &udev) < 0)
      continue;

    char bus_name[4];
    char device_name_str[4];
    if (usb_get_bus_device_strings(dev, bus_name, sizeof(bus_name),
                                   device_name_str,
                                   sizeof(device_name_str)) < 0) {
      usb_close(udev);
      continue;
    }

    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "device found: Bus %s Device %s Name %s", bus_name,
            device_name_str, device_name);
    usb_close(udev);

    if (snprintf(connstrings[device_found], sizeof(nfc_connstring), "%s:%s:%s",
                 ACR122_USB_DRIVER_NAME, bus_name, device_name_str) >=
        (int)sizeof(nfc_connstring)) {
      continue;
    }

    device_found++;
    if (device_found == connstrings_len) {
      usb_free_device_list(&devices);
      return device_found;
    }
  }

  usb_free_device_list(&devices);
  return device_found;
}

struct acr122_usb_descriptor {
  char *dirname;
  char *filename;
};

static bool
acr122_usb_get_usb_device_name(const struct usb_device *dev, usb_dev_handle *udev,
                               char *buffer, size_t len)
{
  if (buffer == NULL || len == 0)
    return false;

  buffer[0] = '\0';

  if ((dev->manufacturer_string_index || dev->product_string_index) && udev) {
    usb_get_string_simple(udev, dev->manufacturer_string_index, buffer, len);
    size_t used = strlen(buffer);
    if (used > 0 && used + 3 < len) {
      memcpy(buffer + used, " / ", 3);
      buffer[used + 3] = '\0';
      used += 3;
    }
    usb_get_string_simple(udev, dev->product_string_index, buffer + used,
                          len - used);
  }

  if (buffer[0] == '\0') {
    const char *device_name = acr122_usb_device_name(dev->vendor_id,
                                                     dev->product_id);
    if (device_name == NULL)
      return false;
    snprintf(buffer, len, "%s", device_name);
  }

  return true;
}

static nfc_device *
acr122_usb_open(const nfc_context *context, const nfc_connstring connstring)
{
  nfc_device *pnd = NULL;
  struct acr122_usb_descriptor desc = {NULL, NULL};
  nfc_connstring fullconnstring;
  bool interface_claimed = false;

  int connstring_decode_level =
      connstring_decode(connstring, ACR122_USB_DRIVER_NAME, "usb", &desc.dirname,
                        &desc.filename);
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "%d element(s) have been decoded from \"%s\"", connstring_decode_level,
          connstring);
  if (connstring_decode_level < 1)
    goto free_mem;

  struct acr122_usb_data data;
  if (nfc_secure_memset(&data, 0, sizeof(data)) < 0)
    goto free_mem;
  data.configuration_value = 1;

  struct usb_device_list devices;
  if (usb_get_device_list(&devices) < 0)
    goto free_mem;

  for (size_t i = 0; i < devices.count; i++) {
    const struct usb_device *dev = &devices.devices[i];
    char bus_name[4];
    char device_name[4];

    if (usb_get_bus_device_strings(dev, bus_name, sizeof(bus_name),
                                   device_name, sizeof(device_name)) < 0) {
      continue;
    }
    if (connstring_decode_level > 1 && strcmp(bus_name, desc.dirname) != 0)
      continue;
    if (connstring_decode_level > 2 && strcmp(device_name, desc.filename) != 0)
      continue;
    if (!acr122_is_usb_device(dev->vendor_id, dev->product_id))
      continue;
    if (!acr122_usb_get_end_points(dev, &data))
      continue;

    if (usb_open(dev, &data.pudh) < 0)
      continue;

    data.configuration_value =
        dev->configuration_value != 0 ? dev->configuration_value : 1;
    usb_reset(data.pudh);

    int res = usb_set_configuration(data.pudh, data.configuration_value);
    if (res < 0) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Unable to set USB configuration (%s)", _usb_strerror(res));
      goto error_with_devices;
    }

    res = usb_claim_interface(data.pudh, data.interface_number);
    if (res < 0) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Unable to claim USB interface (%s)", _usb_strerror(res));
      goto error_with_devices;
    }
    interface_claimed = true;

    if (data.alternate_setting > 0) {
      res = usb_set_altinterface(data.pudh, data.interface_number,
                                 data.alternate_setting);
      if (res < 0) {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Unable to set alternate setting on USB interface (%s)",
                _usb_strerror(res));
        goto error_with_devices;
      }
    }

    snprintf(fullconnstring, sizeof(nfc_connstring), "%s:%s:%s",
             ACR122_USB_DRIVER_NAME, bus_name, device_name);

    pnd = nfc_device_new(context, fullconnstring);
    if (!pnd) {
      perror("malloc");
      goto error_with_devices;
    }

    acr122_usb_get_usb_device_name(dev, data.pudh, pnd->name,
                                   sizeof(pnd->name));

    if (nfc_alloc_driver_data(pnd, sizeof(struct acr122_usb_data)) < 0)
      goto error_with_devices;
    *DRIVER_DATA(pnd) = data;
    DRIVER_DATA(pnd)->abort_flag = false;

    if (pn53x_data_new(pnd, &acr122_usb_io) == NULL) {
      perror("malloc");
      goto error_with_devices;
    }

    acr122_usb_prepare_ccid_header(&DRIVER_DATA(pnd)->tama_frame.ccid_header,
                                   ACR122_CCID_PC_TO_RDR_XFR_BLOCK);
    acr122_usb_prepare_ccid_header(&DRIVER_DATA(pnd)->apdu_frame.ccid_header,
                                   ACR122_CCID_PC_TO_RDR_XFR_BLOCK);

    CHIP_DATA(pnd)->timer_correction = 46;
    pnd->driver = &acr122_usb_driver;

    if (acr122_usb_init(pnd) < 0)
      goto error_with_devices;

    usb_free_device_list(&devices);
    goto free_mem;
  }

  usb_free_device_list(&devices);
  goto free_mem;

error_with_devices:
  if (pnd && pnd->chip_data)
    pn53x_data_free(pnd);
  nfc_device_free(pnd);
  pnd = NULL;
  if (interface_claimed && data.pudh != NULL)
    usb_release_interface(data.pudh, data.interface_number);
  if (data.pudh != NULL)
    usb_close(data.pudh);
  usb_free_device_list(&devices);
free_mem:
  free(desc.dirname);
  free(desc.filename);
  return pnd;
}

static void
acr122_usb_close(nfc_device *pnd)
{
  acr122_usb_ack(pnd);
  pn53x_idle(pnd);

  int res;
  if ((res = usb_release_interface(DRIVER_DATA(pnd)->pudh,
                                   DRIVER_DATA(pnd)->interface_number)) < 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Unable to release USB interface (%s)", _usb_strerror(res));
  }

  usb_close(DRIVER_DATA(pnd)->pudh);
  pn53x_data_free(pnd);
  nfc_device_free(pnd);
}

static int
acr122_build_frame_from_apdu(nfc_device *pnd, const uint8_t ins,
                             const uint8_t p1, const uint8_t p2,
                             const uint8_t *data, const size_t data_len,
                             const uint8_t le)
{
  size_t apdu_len = acr122_build_apdu(
      (uint8_t *)&DRIVER_DATA(pnd)->apdu_frame.apdu_header,
      sizeof(DRIVER_DATA(pnd)->apdu_frame.apdu_header) +
          sizeof(DRIVER_DATA(pnd)->apdu_frame.apdu_payload),
      ins, p1, p2, data, data_len, le);
  if (apdu_len == 0)
    return NFC_EINVARG;

  DRIVER_DATA(pnd)->apdu_frame.ccid_header.dwLength =
      acr122_u32_to_le((uint32_t)apdu_len);
  return (int)(sizeof(struct acr122_ccid_header) + apdu_len);
}

static int
acr122_build_frame_from_tama(nfc_device *pnd, const uint8_t *tama,
                             const size_t tama_len)
{
  size_t apdu_len = acr122_build_direct_transmit_apdu(
      (uint8_t *)&DRIVER_DATA(pnd)->tama_frame.apdu_header,
      sizeof(DRIVER_DATA(pnd)->tama_frame.apdu_header) +
          sizeof(DRIVER_DATA(pnd)->tama_frame.tama_header) +
          sizeof(DRIVER_DATA(pnd)->tama_frame.tama_payload),
      tama, tama_len);
  if (apdu_len == 0)
    return NFC_EINVARG;

  DRIVER_DATA(pnd)->tama_frame.ccid_header.dwLength =
      acr122_u32_to_le((uint32_t)apdu_len);
  return (int)(sizeof(struct acr122_ccid_header) + apdu_len);
}

static int
acr122_usb_send(nfc_device *pnd, const uint8_t *pbtData, const size_t szData,
                const int timeout)
{
  int res = acr122_build_frame_from_tama(pnd, pbtData, szData);
  if (res < 0) {
    pnd->last_error = NFC_EINVARG;
    return pnd->last_error;
  }

  if ((res = acr122_usb_bulk_write(
           DRIVER_DATA(pnd), (unsigned char *)&(DRIVER_DATA(pnd)->tama_frame),
           res, timeout)) < 0) {
    pnd->last_error = res;
    return pnd->last_error;
  }

  return NFC_SUCCESS;
}

#define USB_TIMEOUT_PER_PASS 200
static int
acr122_usb_receive(nfc_device *pnd, uint8_t *pbtData,
                   const size_t szDataLen, const int timeout)
{
  off_t offset = 0;
  uint8_t abtRxBuf[255 + sizeof(struct acr122_ccid_header)];

  int usb_timeout;
  int remaining_time = timeout;

read:
  if (timeout == USB_INFINITE_TIMEOUT) {
    usb_timeout = USB_TIMEOUT_PER_PASS;
  } else {
    remaining_time -= USB_TIMEOUT_PER_PASS;
    if (remaining_time <= 0) {
      pnd->last_error = NFC_ETIMEOUT;
      return pnd->last_error;
    }
    usb_timeout = MIN(remaining_time, USB_TIMEOUT_PER_PASS);
  }

  int res = acr122_usb_bulk_read(DRIVER_DATA(pnd), abtRxBuf, sizeof(abtRxBuf),
                                 usb_timeout);
  size_t len;
  int error;

  if (res == NFC_ETIMEOUT) {
    if (DRIVER_DATA(pnd)->abort_flag) {
      DRIVER_DATA(pnd)->abort_flag = false;
      acr122_usb_ack(pnd);
      pnd->last_error = NFC_EOPABORTED;
      return pnd->last_error;
    }
    goto read;
  }

  if (res < 10) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "Invalid RDR_to_PC_DataBlock frame");
    acr122_usb_ack(pnd);
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  if (abtRxBuf[offset] != ACR122_CCID_RDR_TO_PC_DATABLOCK) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "Frame header mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset++;

  len = abtRxBuf[offset++];
  error = abtRxBuf[8];
  if (len == 0 && error == 0xFE) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s",
            "Command timed out");
    pnd->last_error = NFC_ETIMEOUT;
    return pnd->last_error;
  }

  if (!((len > 1) && (abtRxBuf[10] == ACR122_PN53X_READER_TO_HOST))) {
    struct acr122_status_word status_word;
    if (len != 2 ||
        !acr122_parse_status_words(abtRxBuf + 10, 2, &status_word)) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
              "Wrong reply");
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }

    if (!status_word.has_more_data) {
      if (status_word.application_error) {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
                "PN532 has detected an error at the application level");
      } else if (status_word.no_reply) {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
                "PN532 didn't reply");
      } else {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Unexpected Status Word (SW1: %02x SW2: %02x)",
                status_word.sw1, status_word.sw2);
      }
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }

    res = acr122_usb_send_apdu(pnd, ACR122_APDU_INS_GET_ADDITIONAL_DATA, 0x00,
                               0x00, NULL, 0, status_word.more_data_length,
                               abtRxBuf, sizeof(abtRxBuf));
    if (res == NFC_ETIMEOUT) {
      if (DRIVER_DATA(pnd)->abort_flag) {
        DRIVER_DATA(pnd)->abort_flag = false;
        acr122_usb_ack(pnd);
        pnd->last_error = NFC_EOPABORTED;
        return pnd->last_error;
      }
      goto read;
    }
    if (res < 10) {
      acr122_usb_ack(pnd);
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
  }

  offset = 0;
  if (abtRxBuf[offset] != ACR122_CCID_RDR_TO_PC_DATABLOCK) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "Frame header mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset++;

  len = abtRxBuf[offset++];
  if ((abtRxBuf[offset] != 0x00) || (abtRxBuf[offset + 1] != 0x00) ||
      (abtRxBuf[offset + 2] != 0x00)) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "Not implemented: only 1-byte length is supported, please report this bug with a full trace.");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset += 3;

  if (len < 4) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "Too small reply");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  len -= 4;
  if (len > szDataLen) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Unable to receive data: buffer too small. (szDataLen: %" PRIuPTR
            ", len: %" PRIuPTR ")",
            szDataLen, len);
    pnd->last_error = NFC_EOVFLOW;
    return pnd->last_error;
  }

  offset += 2;
  offset += 2;
  offset += 1;

  if (abtRxBuf[offset] != ACR122_PN53X_READER_TO_HOST) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "TFI Mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset++;

  if (abtRxBuf[offset] != CHIP_DATA(pnd)->last_command + 1) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
            "Command Code verification failed");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset++;

  if (nfc_safe_memcpy(pbtData, szDataLen, abtRxBuf + offset, len) < 0) {
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  return (int)len;
}

int
acr122_usb_ack(nfc_device *pnd)
{
  int res = 0;
  uint8_t acr122_ack_frame[] = {GetFirmwareVersion};
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s", "ACR122 Abort");
  if ((res = acr122_build_frame_from_tama(pnd, acr122_ack_frame,
                                          sizeof(acr122_ack_frame))) < 0) {
    return res;
  }
  if ((res = acr122_usb_bulk_write(
           DRIVER_DATA(pnd), (uint8_t *)&(DRIVER_DATA(pnd)->tama_frame), res,
           1000)) < 0) {
    return res;
  }
  uint8_t abtRxBuf[255 + sizeof(struct acr122_ccid_header)];
  return acr122_usb_bulk_read(DRIVER_DATA(pnd), abtRxBuf, sizeof(abtRxBuf),
                              1000);
}

static int
acr122_usb_send_apdu(nfc_device *pnd, const uint8_t ins, const uint8_t p1,
                     const uint8_t p2, const uint8_t *const data,
                     size_t data_len, const uint8_t le, uint8_t *out,
                     const size_t out_size)
{
  int frame_len =
      acr122_build_frame_from_apdu(pnd, ins, p1, p2, data, data_len, le);
  if (frame_len < 0)
    return frame_len;

  int res = acr122_usb_bulk_write(
      DRIVER_DATA(pnd), (unsigned char *)&(DRIVER_DATA(pnd)->apdu_frame),
      frame_len, 1000);
  if (res < 0)
    return res;

  return acr122_usb_bulk_read(DRIVER_DATA(pnd), out, out_size, 1000);
}

int
acr122_usb_init(nfc_device *pnd)
{
  uint8_t abtRxBuf[255 + sizeof(struct acr122_ccid_header)];

  int res = pn53x_set_property_int(pnd, NP_TIMEOUT_COMMAND, 1000);
  if (res < 0)
    return res;

  uint8_t ccid_frame[] = {
      ACR122_CCID_PC_TO_RDR_ICC_POWER_ON, 0x00, 0x00, 0x00, 0x00,
      0x00,                              0x00, 0x01, 0x00, 0x00,
  };

  if ((res = acr122_usb_bulk_write(DRIVER_DATA(pnd), ccid_frame,
                                   sizeof(struct acr122_ccid_header), 1000)) <
      0) {
    return res;
  }
  if ((res = acr122_usb_bulk_read(DRIVER_DATA(pnd), abtRxBuf, sizeof(abtRxBuf),
                                  1000)) < 0) {
    return res;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s",
          "ACR122 PICC Operating Parameters");
  if ((res = acr122_usb_send_apdu(pnd, 0x00, 0x51, 0x00, NULL, 0, 0, abtRxBuf,
                                  sizeof(abtRxBuf))) < 0) {
    return res;
  }

  res = 0;
  for (int i = 0; i < 3; i++) {
    if (res < 0) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s",
              "PN532 init failed, trying again...");
    }
    if ((res = pn53x_init(pnd)) >= 0)
      break;
  }

  if (res < 0)
    return res;

  return NFC_SUCCESS;
}

static int
acr122_usb_abort_command(nfc_device *pnd)
{
  DRIVER_DATA(pnd)->abort_flag = true;
  return NFC_SUCCESS;
}

const struct pn53x_io acr122_usb_io = {
    .send = acr122_usb_send,
    .receive = acr122_usb_receive,
};

const struct nfc_driver acr122_usb_driver = {
    .name = ACR122_USB_DRIVER_NAME,
    .scan_type = NOT_INTRUSIVE,
    .scan = acr122_usb_scan,
    .open = acr122_usb_open,
    .close = acr122_usb_close,
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

    .abort_command = acr122_usb_abort_command,
    .idle = pn53x_idle,
    .powerdown = NULL,
};
