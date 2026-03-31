/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2017 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
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
 * @file pn53x_usb.c
 * @brief Driver for PN53x using USB
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif // HAVE_CONFIG_H

/*
Thanks to d18c7db and Okko for example code
*/

#include <stdio.h>
#include <stdlib.h>
#include <inttypes.h>
#include <sys/select.h>
#include <errno.h>
#include <string.h>
#ifdef _MSC_VER
#include <sys/types.h>
#endif
#include <nfc/nfc.h>

#include "nfc-internal.h"
#include "nfc-secure.h"
#include "nfc-common.h"
#include "chips/pn53x.h"
#include "chips/pn53x-internal.h"
#include "drivers/pn53x_usb.h"
#include "rust_usb_bridge.h"

#define PN53X_USB_DRIVER_NAME "pn53x_usb"
#define LOG_CATEGORY "libnfc.driver.pn53x_usb"
#define LOG_GROUP NFC_LOG_GROUP_DRIVER

#define USB_INFINITE_TIMEOUT 0

#define DRIVER_DATA(pnd) ((struct pn53x_usb_data *)(pnd->driver_data))

const nfc_modulation_type no_target_support[] = {0};

typedef enum {
  UNKNOWN,
  NXP_PN531,
  SONY_PN531,
  NXP_PN533,
  ASK_LOGO,
  SCM_SCL3711,
  SCM_SCL3712,
  SONY_RCS360
} pn53x_usb_model;

// Internal data struct
struct pn53x_usb_data {
  usb_dev_handle *pudh;
  pn53x_usb_model model;
  uint32_t uiEndPointIn;
  uint32_t uiEndPointOut;
  uint32_t uiMaxPacketSize;
  int interface_number;
  int configuration_value;
  int alternate_setting;
  volatile bool abort_flag;
  bool possibly_corrupted_usbdesc;
};

// Internal io struct
const struct pn53x_io pn53x_usb_io;

// Prototypes
bool pn53x_usb_get_usb_device_name(const struct usb_device *dev,
                                   usb_dev_handle *udev, char *buffer,
                                   size_t len);
int pn53x_usb_init(nfc_device *pnd);

static int
pn53x_usb_bulk_read(struct pn53x_usb_data *data, uint8_t abtRx[], const size_t szRx, const int timeout)
{
  int res = usb_bulk_read(data->pudh, data->uiEndPointIn, abtRx, szRx,
                          timeout);
  if (res > 0) {
    LOG_HEX(NFC_LOG_GROUP_COM, "RX", abtRx, res);
  } else if (res < 0) {
    if (!usb_error_is_timeout(res))
      log_put(NFC_LOG_GROUP_COM, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Unable to read from USB (%s)", _usb_strerror(res));
  }
  return res;
}

static int
pn53x_usb_bulk_write(struct pn53x_usb_data *data, uint8_t abtTx[], const size_t szTx, const int timeout)
{
  LOG_HEX(NFC_LOG_GROUP_COM, "TX", abtTx, szTx);
  int res = usb_bulk_write(data->pudh, data->uiEndPointOut, abtTx, szTx,
                           timeout);
  if (res > 0) {
    // HACK Some hosts expose this descriptor quirk on reconnect.
    if (data->uiMaxPacketSize != 0 && (res % data->uiMaxPacketSize) == 0) {
      usb_bulk_write(data->pudh, data->uiEndPointOut, NULL, 0, timeout);
    }
  } else {
    log_put(NFC_LOG_GROUP_COM, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Unable to write to USB (%s)", _usb_strerror(res));
  }
  return res;
}

struct pn53x_usb_supported_device {
  uint16_t vendor_id;
  uint16_t product_id;
  pn53x_usb_model model;
  const char *name;
  /* hardcoded known values for buggy hardware whose configuration vanishes */
  uint32_t uiEndPointIn;
  uint32_t uiEndPointOut;
  uint32_t uiMaxPacketSize;
};

const struct pn53x_usb_supported_device pn53x_usb_supported_devices[] = {
  {0x04CC, 0x0531, NXP_PN531, "Philips / PN531", 0x84, 0x04, 0x40},
  {0x04CC, 0x2533, NXP_PN533, "NXP / PN533", 0x84, 0x04, 0x40},
  {0x04E6, 0x5591, SCM_SCL3711, "SCM Micro / SCL3711-NFC&RW", 0x84, 0x04, 0x40},
  {0x04E6, 0x5594, SCM_SCL3712, "SCM Micro / SCL3712-NFC&RW", 0, 0, 0}, // to check on real device
  {0x054c, 0x0193, SONY_PN531, "Sony / PN531", 0x84, 0x04, 0x40},
  {0x1FD3, 0x0608, ASK_LOGO, "ASK / LoGO", 0x84, 0x04, 0x40},
  {0x054C, 0x02E1, SONY_RCS360, "Sony / FeliCa S360 [PaSoRi]", 0x84, 0x04, 0x40}
};

// PN533 USB descriptors backup buffers

const uint8_t btXramUsbDesc_scl3711[] = {
  0x09,
  0x02,
  0x20,
  0x00,
  0x01,
  0x01,
  0x00,
  0x80,
  0x32,
  0x09,
  0x04,
  0x00,
  0x00,
  0x02,
  0xff,
  0xff,
  0xff,
  0x00,
  0x07,
  0x05,
  0x04,
  0x02,
  0x40,
  0x00,
  0x04,
  0x07,
  0x05,
  0x84,
  0x02,
  0x40,
  0x00,
  0x04,
  0x1e,
  0x03,
  0x53,
  0x00,
  0x43,
  0x00,
  0x4c,
  0x00,
  0x33,
  0x00,
  0x37,
  0x00,
  0x31,
  0x00,
  0x31,
  0x00,
  0x2d,
  0x00,
  0x4e,
  0x00,
  0x46,
  0x00,
  0x43,
  0x00,
  0x26,
  0x00,
  0x52,
  0x00,
  0x57,
};
const uint8_t btXramUsbDesc_nxppn533[] = {
  0x09,
  0x02,
  0x20,
  0x00,
  0x01,
  0x01,
  0x00,
  0x80,
  0x32,
  0x09,
  0x04,
  0x00,
  0x00,
  0x02,
  0xff,
  0xff,
  0xff,
  0x00,
  0x07,
  0x05,
  0x04,
  0x02,
  0x40,
  0x00,
  0x04,
  0x07,
  0x05,
  0x84,
  0x02,
  0x40,
  0x00,
  0x04,
  0x0c,
  0x03,
  0x50,
  0x00,
  0x4e,
  0x00,
  0x35,
  0x00,
  0x33,
  0x00,
  0x33,
  0x00,
  0x04,
  0x03,
  0x09,
  0x04,
  0x08,
  0x03,
  0x4e,
  0x00,
  0x58,
  0x00,
  0x50,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
};
const uint8_t btXramUsbDesc_asklogo[] = {
  0x09,
  0x02,
  0x20,
  0x00,
  0x01,
  0x01,
  0x00,
  0x80,
  0x96,
  0x09,
  0x04,
  0x00,
  0x00,
  0x02,
  0xff,
  0xff,
  0xff,
  0x00,
  0x07,
  0x05,
  0x04,
  0x02,
  0x40,
  0x00,
  0x04,
  0x07,
  0x05,
  0x84,
  0x02,
  0x40,
  0x00,
  0x04,
  0x0a,
  0x03,
  0x4c,
  0x00,
  0x6f,
  0x00,
  0x47,
  0x00,
  0x4f,
  0x00,
  0x04,
  0x03,
  0x09,
  0x04,
  0x08,
  0x03,
  0x41,
  0x00,
  0x53,
  0x00,
  0x4b,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
  0x00,
};

static void pn533_fix_usbdesc(nfc_device *pnd)
{
  // PN533 USB descriptors may have been corrupted by large commands/responses
  // so they need to be restored before closing usb connection.
  // cf PN5331B3HNC270 Release Note
  uint32_t szXramUsbDesc = 0;
  uint8_t *btXramUsbDesc = NULL;
  if (DRIVER_DATA(pnd)->model == NXP_PN533) {
    btXramUsbDesc = (uint8_t *)btXramUsbDesc_nxppn533;
    szXramUsbDesc = sizeof(btXramUsbDesc_nxppn533);
  } else if (DRIVER_DATA(pnd)->model == SCM_SCL3711) {
    btXramUsbDesc = (uint8_t *)btXramUsbDesc_scl3711;
    szXramUsbDesc = sizeof(btXramUsbDesc_scl3711);
  } else if (DRIVER_DATA(pnd)->model == ASK_LOGO) {
    btXramUsbDesc = (uint8_t *)btXramUsbDesc_asklogo;
    szXramUsbDesc = sizeof(btXramUsbDesc_asklogo);
  }
#define MAXSZXRAMUSBDESC 61
  if ((szXramUsbDesc == 0) || (MAXSZXRAMUSBDESC > 61))
    return;
#if 0
  // Debug routine to check if corruption occurred:
  // Don't read more regs at once or it will trigger the bug and corrupt what we're busy reading!
  uint8_t abtCmdRR[] = { ReadRegister, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 };
  uint8_t nRRreg = ((sizeof(abtCmdRR) - 1) / 2);
  uint8_t abtRxRR[1 + nRRreg];
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "%s", "Checking USB descriptors corruption in XRAM");
  for (uint8_t i = 0x19, j = 0; i < 0x19 + szXramUsbDesc;) {
    for (uint8_t k = 0; k < nRRreg; k++) {
      abtCmdRR[(2 * k) + 2] = i++;
    }
    if (pn53x_transceive(pnd, abtCmdRR, sizeof(abtCmdRR), abtRxRR, sizeof(abtRxRR), -1) < 0) {
      return;  // void
    }
    for (int k = 0; (k < nRRreg) && (j < szXramUsbDesc); k++) {
      //printf("0x%02x, ", abtRxRR[1 + k]);
      if (btXramUsbDesc[j] != abtRxRR[1 + k])
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "XRAM corruption @ addr 0x00%02X: got %02x, expected %02x", 0x0019 + (j - 1), abtRxRR[1 + k], btXramUsbDesc[j]);
      j++;
    }
  }
#endif
  // Abuse the overflow bug to restore USB descriptors in one go
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "%s", "Fixing USB descriptors corruption");
  uint8_t abtCmdWR[19 + MAXSZXRAMUSBDESC] = {GetFirmwareVersion};
  for (uint8_t i = 0; i < szXramUsbDesc; i++) {
    abtCmdWR[i + 19] = btXramUsbDesc[i];
  }
  size_t szCmdWR = sizeof(abtCmdWR);
  uint8_t abtRxWR[4];
  if (pn53x_transceive(pnd, abtCmdWR, szCmdWR, abtRxWR, sizeof(abtRxWR), -1) < 0) {
    return; // void
  }
  DRIVER_DATA(pnd)->possibly_corrupted_usbdesc = false;
}

static pn53x_usb_model
pn53x_usb_get_device_model(uint16_t vendor_id, uint16_t product_id)
{
  for (size_t n = 0; n < sizeof(pn53x_usb_supported_devices) / sizeof(struct pn53x_usb_supported_device); n++) {
    if ((vendor_id == pn53x_usb_supported_devices[n].vendor_id) &&
        (product_id == pn53x_usb_supported_devices[n].product_id))
      return pn53x_usb_supported_devices[n].model;
  }

  return UNKNOWN;
}

static bool
pn53x_usb_get_end_points_default(const struct usb_device *dev,
                                 struct pn53x_usb_data *data)
{
  for (size_t n = 0; n < sizeof(pn53x_usb_supported_devices) / sizeof(struct pn53x_usb_supported_device); n++) {
    if ((dev->vendor_id == pn53x_usb_supported_devices[n].vendor_id) &&
        (dev->product_id == pn53x_usb_supported_devices[n].product_id)) {
      if (pn53x_usb_supported_devices[n].uiMaxPacketSize != 0) {
        data->uiEndPointIn = pn53x_usb_supported_devices[n].uiEndPointIn;
        data->uiEndPointOut = pn53x_usb_supported_devices[n].uiEndPointOut;
        data->uiMaxPacketSize = pn53x_usb_supported_devices[n].uiMaxPacketSize;

        return true;
      }
    }
  }

  return false;
}

int pn53x_usb_ack(nfc_device *pnd);

static bool
pn53x_usb_get_end_points(const struct usb_device *dev,
                         struct pn53x_usb_data *data)
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
pn53x_usb_scan(const nfc_context *context, nfc_connstring connstrings[], const size_t connstrings_len)
{
  (void)context;

  struct usb_device_list devices;
  if (usb_get_device_list(&devices) < 0)
    return 0;

  size_t device_found = 0;
  for (size_t i = 0; i < devices.count; i++) {
    const struct usb_device *dev = &devices.devices[i];
    for (size_t n = 0; n < sizeof(pn53x_usb_supported_devices) / sizeof(struct pn53x_usb_supported_device); n++) {
      if ((pn53x_usb_supported_devices[n].vendor_id == dev->vendor_id) &&
          (pn53x_usb_supported_devices[n].product_id == dev->product_id)) {
        if (pn53x_usb_supported_devices[n].uiMaxPacketSize == 0) {
          struct usb_bulk_endpoints endpoints;
          if (!usb_device_get_bulk_endpoints(dev, &endpoints))
            continue;
        }

        usb_dev_handle *udev = NULL;
        if (usb_open(dev, &udev) < 0)
          continue;

        int res = usb_set_configuration(
            udev, dev->configuration_value != 0 ? dev->configuration_value : 1);
        if (res < 0) {
          log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                  "Unable to set USB configuration (%s)",
                  _usb_strerror(res));
          usb_close(udev);
          continue;
        }

        char bus_name[4];
        char device_name[4];
        if (usb_get_bus_device_strings(dev, bus_name, sizeof(bus_name),
                                       device_name, sizeof(device_name)) < 0) {
          usb_close(udev);
          continue;
        }

        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
                "device found: Bus %s Device %s", bus_name, device_name);
        usb_close(udev);
        if (snprintf(connstrings[device_found], sizeof(nfc_connstring), "%s:%s:%s", PN53X_USB_DRIVER_NAME, bus_name, device_name) >= (int)sizeof(nfc_connstring)) {
          continue;
        }
        device_found++;
        if (device_found == connstrings_len) {
          usb_free_device_list(&devices);
          return device_found;
        }
      }
    }
  }

  usb_free_device_list(&devices);
  return device_found;
}

struct pn53x_usb_descriptor {
  char *dirname;
  char *filename;
};

bool pn53x_usb_get_usb_device_name(const struct usb_device *dev,
                                   usb_dev_handle *udev, char *buffer,
                                   size_t len)
{
  *buffer = '\0';

  if (dev->manufacturer_string_index || dev->product_string_index) {
    if (udev) {
      usb_get_string_simple(udev, dev->manufacturer_string_index, buffer, len);
      /* Safely determine buffer length with bounds check (CWE-126) */
      if (nfc_safe_strlen(buffer, len) > 0) {
        size_t current_len = nfc_safe_strlen(buffer, len);
        const char *separator = " / ";
        const size_t sep_len = 3; /* strlen(" / ") is constant */
        if (current_len + sep_len < len) {
          if (nfc_safe_memcpy(buffer + current_len, len - current_len, separator, sep_len) < 0) {
            log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to append separator");
            return false;
          }
          buffer[current_len + sep_len] = '\0';
        }
      }
      /* Safe: buffer is guaranteed null-terminated by usb_get_string_simple and our checks above */
      size_t current_buffer_len = nfc_safe_strlen(buffer, len);
      usb_get_string_simple(udev, dev->product_string_index, buffer + current_buffer_len, len - current_buffer_len);
    }
  }

  if (!*buffer) {
    for (size_t n = 0; n < sizeof(pn53x_usb_supported_devices) / sizeof(struct pn53x_usb_supported_device); n++) {
      if ((pn53x_usb_supported_devices[n].vendor_id == dev->vendor_id) &&
          (pn53x_usb_supported_devices[n].product_id == dev->product_id)) {
        /* Safely determine device name length (CWE-126) */
        size_t name_len = nfc_safe_strlen(pn53x_usb_supported_devices[n].name, 256);
        size_t copy_len = (name_len < len - 1) ? name_len : (len - 1);
        if (nfc_safe_memcpy(buffer, len, pn53x_usb_supported_devices[n].name, copy_len) < 0) {
          log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy device name");
          return false;
        }
        buffer[copy_len] = '\0';
        return true;
      }
    }
  }

  return false;
}

static nfc_device *
pn53x_usb_open(const nfc_context *context, const nfc_connstring connstring)
{
  nfc_device *pnd = NULL;
  struct pn53x_usb_descriptor desc = {NULL, NULL};
  nfc_connstring fullconnstring;
  bool interface_claimed = false;
  int connstring_decode_level = connstring_decode(connstring, PN53X_USB_DRIVER_NAME, "usb", &desc.dirname, &desc.filename);
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%d element(s) have been decoded from \"%s\"", connstring_decode_level, connstring);
  if (connstring_decode_level < 1) {
    goto free_mem;
  }

  struct pn53x_usb_data data = {
    .pudh = NULL,
    .uiEndPointIn = 0,
    .uiEndPointOut = 0,
    .uiMaxPacketSize = 0,
    .interface_number = 0,
    .configuration_value = 1,
    .alternate_setting = 0,
    .possibly_corrupted_usbdesc = false,
  };
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
    if (pn53x_usb_get_device_model(dev->vendor_id, dev->product_id) == UNKNOWN)
      continue;

    if (usb_open(dev, &data.pudh) < 0)
      continue;

    data.configuration_value =
        dev->configuration_value != 0 ? dev->configuration_value : 1;
    if (pn53x_usb_get_end_points_default(dev, &data) == false &&
        pn53x_usb_get_end_points(dev, &data) == false) {
      usb_close(data.pudh);
      data.pudh = NULL;
      continue;
    }

    int res = usb_set_configuration(data.pudh, data.configuration_value);
    if (res < 0) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Unable to set USB configuration (%s)", _usb_strerror(res));
      if (usb_error_is_access(res)) {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "Warning: Please double check USB permissions for device %04x:%04x", dev->vendor_id, dev->product_id);
      }
      goto error_with_devices;
    }

    res = usb_claim_interface(data.pudh, data.interface_number);
    if (res < 0) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Unable to claim USB interface (%s)", _usb_strerror(res));
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

    data.model = pn53x_usb_get_device_model(dev->vendor_id, dev->product_id);

    if (snprintf(fullconnstring, sizeof(nfc_connstring), "%s:%s:%s",
                 PN53X_USB_DRIVER_NAME, bus_name, device_name) >=
        (int)sizeof(nfc_connstring)) {
      goto error_with_devices;
    }

    pnd = nfc_device_new(context, fullconnstring);
    if (!pnd) {
      perror("malloc");
      goto error_with_devices;
    }
    pn53x_usb_get_usb_device_name(dev, data.pudh, pnd->name, sizeof(pnd->name));

    if (nfc_alloc_driver_data(pnd, sizeof(struct pn53x_usb_data)) < 0) {
      goto error_with_devices;
    }
    *DRIVER_DATA(pnd) = data;

    if (pn53x_data_new(pnd, &pn53x_usb_io) == NULL) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Failed to allocate chip data");
      goto error_with_devices;
    }

    switch (DRIVER_DATA(pnd)->model) {
      case ASK_LOGO:
        CHIP_DATA(pnd)->timer_correction = 50;
        CHIP_DATA(pnd)->progressive_field = true;
        break;
      case SCM_SCL3711:
      case SCM_SCL3712:
      case NXP_PN533:
        CHIP_DATA(pnd)->timer_correction = 46;
        break;
      case NXP_PN531:
        CHIP_DATA(pnd)->timer_correction = 50;
        break;
      case SONY_PN531:
        CHIP_DATA(pnd)->timer_correction = 54;
        break;
      case SONY_RCS360:
      case UNKNOWN:
        CHIP_DATA(pnd)->timer_correction = 0;
        break;
    }
    pnd->driver = &pn53x_usb_driver;

    pn53x_usb_ack(pnd);

    if (pn53x_usb_init(pnd) < 0) {
      goto error_with_devices;
    }
    DRIVER_DATA(pnd)->abort_flag = false;
    usb_free_device_list(&devices);
    goto free_mem;
  }
  usb_free_device_list(&devices);
  goto free_mem;

error_with_devices:
  if (interface_claimed && data.pudh != NULL) {
    usb_release_interface(data.pudh, data.interface_number);
  }
  if (data.pudh != NULL) {
    usb_close(data.pudh);
    data.pudh = NULL;
  }
  usb_free_device_list(&devices);
  if (pnd != NULL && pnd->chip_data != NULL)
    pn53x_data_free(pnd);
  nfc_device_free(pnd);
  pnd = NULL;
free_mem:
  free(desc.dirname);
  free(desc.filename);
  return pnd;
}

static void
pn53x_usb_close(nfc_device *pnd)
{
  pn53x_usb_ack(pnd);

  if (DRIVER_DATA(pnd)->model == ASK_LOGO) {
    /* Set P30, P31, P32, P33, P35 to logic 1 and P34 to 0 logic */
    /* ie. Switch all LEDs off and turn off progressive field */
    pn53x_write_register(pnd, PN53X_SFR_P3, 0xFF, _BV(P30) | _BV(P31) | _BV(P32) | _BV(P33) | _BV(P35));
  }

  if (DRIVER_DATA(pnd)->possibly_corrupted_usbdesc)
    pn533_fix_usbdesc(pnd);

  pn53x_idle(pnd);

  int res;
  if ((res = usb_release_interface(DRIVER_DATA(pnd)->pudh,
                                   DRIVER_DATA(pnd)->interface_number)) < 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Unable to release USB interface (%s)", _usb_strerror(res));
  }

  usb_close(DRIVER_DATA(pnd)->pudh);
  pn53x_data_free(pnd);
  nfc_device_free(pnd);
}

#define PN53X_USB_BUFFER_LEN (PN53x_EXTENDED_FRAME__DATA_MAX_LEN + PN53x_EXTENDED_FRAME__OVERHEAD)

static int
pn53x_usb_send(nfc_device *pnd, const uint8_t *pbtData, const size_t szData, const int timeout)
{
  uint8_t abtFrame[PN53X_USB_BUFFER_LEN] = {0x00, 0x00, 0xff}; // Every packet must start with "00 00 ff"
  size_t szFrame = 0;
  int res = 0;

  if ((res = pn53x_build_frame(abtFrame, &szFrame, pbtData, szData)) < 0) {
    pnd->last_error = res;
    return pnd->last_error;
  }

  DRIVER_DATA(pnd)->possibly_corrupted_usbdesc |= szData > 17;
  if ((res = pn53x_usb_bulk_write(DRIVER_DATA(pnd), abtFrame, szFrame, timeout)) < 0) {
    pnd->last_error = res;
    return pnd->last_error;
  }

  uint8_t abtRxBuf[PN53X_USB_BUFFER_LEN];
  if ((res = pn53x_usb_bulk_read(DRIVER_DATA(pnd), abtRxBuf, sizeof(abtRxBuf), timeout)) < 0) {
    // try to interrupt current device state
    pn53x_usb_ack(pnd);
    pnd->last_error = res;
    return pnd->last_error;
  }

  if (pn53x_check_ack_frame(pnd, abtRxBuf, res) == 0) {
    // The PN53x is running the sent command
  } else {
    // For some reasons (eg. send another command while a previous one is
    // running), the PN533 sometimes directly replies the response packet
    // instead of ACK frame, so we send a NACK frame to force PN533 to resend
    // response packet. With this hack, the next executed function (ie.
    // pn53x_usb_receive()) will be able to retrieve the correct response
    // packet.
    // FIXME Sony reader is also affected by this bug but NACK is not supported
    if ((res = pn53x_usb_bulk_write(DRIVER_DATA(pnd), (uint8_t *)pn53x_nack_frame, sizeof(pn53x_nack_frame), timeout)) < 0) {
      pnd->last_error = res;
      // try to interrupt current device state
      pn53x_usb_ack(pnd);
      return pnd->last_error;
    }
  }
  return NFC_SUCCESS;
}

#define USB_TIMEOUT_PER_PASS 200
static int
pn53x_usb_receive(nfc_device *pnd, uint8_t *pbtData, const size_t szDataLen, const int timeout)
{
  size_t len;
  off_t offset = 0;

  uint8_t abtRxBuf[PN53X_USB_BUFFER_LEN];
  int res;

  /*
   * If no timeout is specified but the command is blocking, force a 200ms (USB_TIMEOUT_PER_PASS)
   * timeout to allow breaking the loop if the user wants to stop it.
   */
  int usb_timeout;
  int remaining_time = timeout;
read:
  if (timeout == USB_INFINITE_TIMEOUT) {
    usb_timeout = USB_TIMEOUT_PER_PASS;
  } else {
    // A user-provided timeout is set, we have to cut it in multiple chunk to be able to keep an nfc_abort_command() mechanism
    remaining_time -= USB_TIMEOUT_PER_PASS;
    if (remaining_time <= 0) {
      pnd->last_error = NFC_ETIMEOUT;
      return pnd->last_error;
    } else {
      usb_timeout = MIN(remaining_time, USB_TIMEOUT_PER_PASS);
    }
  }

  res = pn53x_usb_bulk_read(DRIVER_DATA(pnd), abtRxBuf, sizeof(abtRxBuf), usb_timeout);

  if (usb_error_is_timeout(res)) {
    if (DRIVER_DATA(pnd)->abort_flag) {
      DRIVER_DATA(pnd)->abort_flag = false;
      pn53x_usb_ack(pnd);
      pnd->last_error = NFC_EOPABORTED;
      return pnd->last_error;
    } else {
      goto read;
    }
  }

  if (res < 0) {
    // try to interrupt current device state
    pn53x_usb_ack(pnd);
    pnd->last_error = res;
    return pnd->last_error;
  }

  const uint8_t pn53x_preamble[3] = {0x00, 0x00, 0xff};
  if (0 != (memcmp(abtRxBuf, pn53x_preamble, 3))) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Frame preamble+start code mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset += 3;

  if ((0x01 == abtRxBuf[offset]) && (0xff == abtRxBuf[offset + 1])) {
    // Error frame
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Application level error detected");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  } else if ((0xff == abtRxBuf[offset]) && (0xff == abtRxBuf[offset + 1])) {
    // Extended frame
    offset += 2;

    // (abtRxBuf[offset] << 8) + abtRxBuf[offset + 1] (LEN) include TFI + (CC+1)
    len = (abtRxBuf[offset] << 8) + abtRxBuf[offset + 1] - 2;
    if (((abtRxBuf[offset] + abtRxBuf[offset + 1] + abtRxBuf[offset + 2]) % 256) != 0) {
      // TODO: Retry
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Length checksum mismatch");
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }
    offset += 3;
  } else {
    // Normal frame
    if (256 != (abtRxBuf[offset] + abtRxBuf[offset + 1])) {
      // TODO: Retry
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Length checksum mismatch");
      pnd->last_error = NFC_EIO;
      return pnd->last_error;
    }

    // abtRxBuf[3] (LEN) include TFI + (CC+1)
    len = abtRxBuf[offset] - 2;
    offset += 2;
  }

  if (len > szDataLen) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Unable to receive data: buffer too small. (szDataLen: %" PRIuPTR ", len: %" PRIuPTR ")", szDataLen, len);
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }

  // TFI + PD0 (CC+1)
  if (abtRxBuf[offset] != 0xD5) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "TFI Mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset += 1;

  if (abtRxBuf[offset] != CHIP_DATA(pnd)->last_command + 1) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Command Code verification failed");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset += 1;

  if (nfc_safe_memcpy(pbtData, szDataLen, abtRxBuf + offset, len) < 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy USB receive data");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset += len;

  uint8_t btDCS = (256 - 0xD5);
  btDCS -= CHIP_DATA(pnd)->last_command + 1;
  for (size_t szPos = 0; szPos < len; szPos++) {
    btDCS -= pbtData[szPos];
  }

  if (btDCS != abtRxBuf[offset]) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Data checksum mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  offset += 1;

  if (0x00 != abtRxBuf[offset]) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Frame postamble mismatch");
    pnd->last_error = NFC_EIO;
    return pnd->last_error;
  }
  // The PN53x command is done and we successfully received the reply
  pnd->last_error = 0;
  DRIVER_DATA(pnd)->possibly_corrupted_usbdesc |= len > 16;
  return len;
}

int pn53x_usb_ack(nfc_device *pnd)
{
  return pn53x_usb_bulk_write(DRIVER_DATA(pnd), (uint8_t *)pn53x_ack_frame, sizeof(pn53x_ack_frame), 1000);
}

int pn53x_usb_init(nfc_device *pnd)
{
  int res = 0;
  // Sometimes PN53x USB doesn't reply ACK one the first frame, so we need to send a dummy one...
  // pn53x_check_communication (pnd); // Sony RC-S360 doesn't support this command for now so let's use a get_firmware_version instead:
  const uint8_t abtCmd[] = {GetFirmwareVersion};
  pn53x_transceive(pnd, abtCmd, sizeof(abtCmd), NULL, 0, -1);
  // ...and we don't care about error
  pnd->last_error = 0;
  if (SONY_RCS360 == DRIVER_DATA(pnd)->model) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s", "SONY RC-S360 initialization.");
    const uint8_t abtCmd2[] = {0x18, 0x01};
    pn53x_transceive(pnd, abtCmd2, sizeof(abtCmd2), NULL, 0, -1);
    pn53x_usb_ack(pnd);
  }

  if ((res = pn53x_init(pnd)) < 0)
    return res;

  if (ASK_LOGO == DRIVER_DATA(pnd)->model) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%s", "ASK LoGO initialization.");
    /* Internal registers */
    /* Disable 100mA current limit, Power on Secure IC (SVDD) */
    pn53x_write_register(pnd, PN53X_REG_Control_switch_rng, 0xFF, SYMBOL_CURLIMOFF | SYMBOL_SIC_SWITCH_EN | SYMBOL_RANDOM_DATAREADY);
    /* Select the signal to be output on SIGOUT: Modulation signal (envelope) from the internal coder */
    pn53x_write_register(pnd, PN53X_REG_CIU_TxSel, 0xFF, 0x14);

    /* SFR Registers */
    /* Setup push-pulls for pins from P30 to P35 */
    pn53x_write_register(pnd, PN53X_SFR_P3CFGB, 0xFF, 0x37);

    /*
    On ASK LoGO hardware:
      LEDs port bits definition:
       * LED 1: bit 2 (P32)
       * LED 2: bit 1 (P31)
       * LED 3: bit 0 or 3 (depending of hardware revision) (P30 or P33)
       * LED 4: bit 5 (P35)
      Notes:
       * Set logical 0 to switch LED on; logical 1 to switch LED off.
       * Bit 4 should be maintained at 1 to keep RF field on.

      Progressive field activation:
       The ASK LoGO hardware can progressively power-up the antenna.
       To use this feature we have to switch on the field by switching on
       the field on PN533 (RFConfiguration) then set P34 to '1', and cut-off the
       field by switching off the field on PN533 then set P34 to '0'.
    */

    /* Set P30, P31, P33, P35 to logic 1 and P32, P34 to 0 logic */
    /* ie. Switch LED1 on and turn off progressive field */
    pn53x_write_register(pnd, PN53X_SFR_P3, 0xFF, _BV(P30) | _BV(P31) | _BV(P33) | _BV(P35));
  }
  if (DRIVER_DATA(pnd)->possibly_corrupted_usbdesc)
    pn533_fix_usbdesc(pnd);

  return NFC_SUCCESS;
}

static int
pn53x_usb_set_property_bool(nfc_device *pnd, const nfc_property property, const bool bEnable)
{
  int res = 0;
  if ((res = pn53x_set_property_bool(pnd, property, bEnable)) < 0)
    return res;

  switch (DRIVER_DATA(pnd)->model) {
    case ASK_LOGO:
      if (NP_ACTIVATE_FIELD == property) {
        /* Switch on/off LED2 and Progressive Field GPIO according to ACTIVATE_FIELD option */
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Switch progressive field %s", bEnable ? "On" : "Off");
        if (pn53x_write_register(pnd, PN53X_SFR_P3, _BV(P31) | _BV(P34), bEnable ? _BV(P34) : _BV(P31)) < 0)
          return NFC_ECHIP;
      }
      break;
    case SCM_SCL3711:
      if (NP_ACTIVATE_FIELD == property) {
        // Switch on/off LED according to ACTIVATE_FIELD option
        if ((res = pn53x_write_register(pnd, PN53X_SFR_P3, _BV(P32), bEnable ? 0 : _BV(P32))) < 0)
          return res;
      }
      break;
    case SCM_SCL3712:
      if (NP_ACTIVATE_FIELD == property) {
        // Switch on/off LED according to ACTIVATE_FIELD option
        if ((res = pn53x_write_register(pnd, PN53X_SFR_P3, _BV(P32), bEnable ? 0 : _BV(P32))) < 0)
          return res;
      }
      break;
    case NXP_PN531:
    case NXP_PN533:
    case SONY_PN531:
    case SONY_RCS360:
    case UNKNOWN:
      // Nothing to do.
      break;
  }
  return NFC_SUCCESS;
}

static int
pn53x_usb_abort_command(nfc_device *pnd)
{
  DRIVER_DATA(pnd)->abort_flag = true;
  return NFC_SUCCESS;
}

static int
pn53x_usb_get_supported_modulation(nfc_device *pnd, const nfc_mode mode, const nfc_modulation_type **const supported_mt)
{
  if ((DRIVER_DATA(pnd)->model != ASK_LOGO) || (mode != N_TARGET))
    return pn53x_get_supported_modulation(pnd, mode, supported_mt);
  else // ASK_LOGO has no N_TARGET support
    *supported_mt = no_target_support;
  return NFC_SUCCESS;
}

const struct pn53x_io pn53x_usb_io = {
  .send = pn53x_usb_send,
  .receive = pn53x_usb_receive,
};

const struct nfc_driver pn53x_usb_driver = {
  .name = PN53X_USB_DRIVER_NAME,
  .scan_type = NOT_INTRUSIVE,
  .scan = pn53x_usb_scan,
  .open = pn53x_usb_open,
  .close = pn53x_usb_close,
  .strerror = pn53x_strerror,

  .initiator_init = pn53x_initiator_init,
  .initiator_init_secure_element = NULL, // No secure-element support
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

  .device_set_property_bool = pn53x_usb_set_property_bool,
  .device_set_property_int = pn53x_set_property_int,
  .get_supported_modulation = pn53x_usb_get_supported_modulation,
  .get_supported_baud_rate = pn53x_get_supported_baud_rate,
  .device_get_information_about = pn53x_get_information_about,

  .abort_command = pn53x_usb_abort_command,
  .idle = pn53x_idle,
  .powerdown = pn53x_PowerDown,
};
