/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2013 Philippe Teuwen
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
 *
 */

/**
 * @file usbbus.c
 * @brief libusb-1.0 helper layer for USB-backed NFC drivers
 */

#ifdef HAVE_CONFIG_H
#  include "config.h"
#endif // HAVE_CONFIG_H

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libusb.h>

#include "usbbus.h"
#include "log.h"
#define LOG_CATEGORY "libnfc.buses.usbbus"
#define LOG_GROUP    NFC_LOG_GROUP_DRIVER

struct usb_dev_handle {
  libusb_device_handle *handle;
};

static libusb_context *g_usb_context = NULL;

enum {
  USB_ENDPOINT_TYPE_MASK = 0x03,
  USB_ENDPOINT_TYPE_BULK = 0x02,
  USB_ENDPOINT_DIR_MASK = 0x80,
  USB_ENDPOINT_IN = 0x80,
  USB_ENDPOINT_OUT = 0x00,
};

static void
usb_free_device(struct usb_device *device)
{
  if (device == NULL)
    return;

  for (size_t i = 0; i < device->interface_count; i++) {
    free(device->interfaces[i].endpoints);
    device->interfaces[i].endpoints = NULL;
    device->interfaces[i].endpoint_count = 0;
  }
  free(device->interfaces);
  device->interfaces = NULL;
  device->interface_count = 0;

  if (device->native_device != NULL) {
    libusb_unref_device((libusb_device *)device->native_device);
    device->native_device = NULL;
  }
}

static int
usb_copy_config_descriptor(struct usb_device *device,
                           const struct libusb_config_descriptor *config)
{
  if (device == NULL || config == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  device->configuration_value = config->bConfigurationValue;
  device->interface_count = config->bNumInterfaces;
  if (device->interface_count == 0)
    return 0;

  device->interfaces = calloc(device->interface_count,
                              sizeof(struct usb_interface_descriptor));
  if (device->interfaces == NULL)
    return LIBUSB_ERROR_NO_MEM;

  for (size_t i = 0; i < device->interface_count; i++) {
    const struct libusb_interface *interface_desc = &config->interface[i];
    if (interface_desc->num_altsetting == 0)
      continue;

    const struct libusb_interface_descriptor *altsetting =
        &interface_desc->altsetting[0];
    struct usb_interface_descriptor *dst = &device->interfaces[i];
    dst->number = altsetting->bInterfaceNumber;
    dst->alternate_setting = altsetting->bAlternateSetting;
    dst->endpoint_count = altsetting->bNumEndpoints;
    if (dst->endpoint_count == 0)
      continue;

    dst->endpoints = calloc(dst->endpoint_count,
                            sizeof(struct usb_endpoint_descriptor));
    if (dst->endpoints == NULL)
      return LIBUSB_ERROR_NO_MEM;

    for (size_t j = 0; j < dst->endpoint_count; j++) {
      dst->endpoints[j].address = altsetting->endpoint[j].bEndpointAddress;
      dst->endpoints[j].attributes = altsetting->endpoint[j].bmAttributes;
      dst->endpoints[j].max_packet_size =
          altsetting->endpoint[j].wMaxPacketSize;
    }
  }

  return 0;
}

int
usb_prepare(void)
{
  if (g_usb_context == NULL) {
    int res = libusb_init(&g_usb_context);
    if (res < 0) {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Unable to initialize libusb (%s)", usb_strerror(res));
      return res;
    }

#ifdef ENVVARS
    char *env_log_level = getenv("LIBNFC_LOG_LEVEL");
    if (env_log_level &&
        (((atoi(env_log_level) >> (NFC_LOG_GROUP_LIBUSB * 2)) &
          0x00000003) >= NFC_LOG_PRIORITY_DEBUG)) {
#ifdef LIBUSB_OPTION_LOG_LEVEL
      libusb_set_option(g_usb_context, LIBUSB_OPTION_LOG_LEVEL,
                        LIBUSB_LOG_LEVEL_DEBUG);
#endif
    }
#endif
  }

  return 0;
}

int
usb_get_device_list(struct usb_device_list *list)
{
  if (list == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  list->devices = NULL;
  list->count = 0;

  int res = usb_prepare();
  if (res < 0)
    return res;

  libusb_device **native_devices = NULL;
  ssize_t device_count = libusb_get_device_list(g_usb_context, &native_devices);
  if (device_count < 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Unable to enumerate USB devices (%s)",
            usb_strerror((int)device_count));
    return (int)device_count;
  }

  if (device_count == 0) {
    libusb_free_device_list(native_devices, 1);
    return 0;
  }

  list->devices = calloc((size_t)device_count, sizeof(struct usb_device));
  if (list->devices == NULL) {
    libusb_free_device_list(native_devices, 1);
    return LIBUSB_ERROR_NO_MEM;
  }

  for (ssize_t i = 0; i < device_count; i++) {
    struct libusb_device_descriptor descriptor;
    res = libusb_get_device_descriptor(native_devices[i], &descriptor);
    if (res < 0)
      continue;

    struct usb_device *dst = &list->devices[list->count];
    dst->native_device = libusb_ref_device(native_devices[i]);
    dst->vendor_id = descriptor.idVendor;
    dst->product_id = descriptor.idProduct;
    dst->manufacturer_string_index = descriptor.iManufacturer;
    dst->product_string_index = descriptor.iProduct;
    dst->bus_number = libusb_get_bus_number(native_devices[i]);
    dst->device_address = libusb_get_device_address(native_devices[i]);
    dst->configuration_value = 1;

    struct libusb_config_descriptor *config = NULL;
    res = libusb_get_config_descriptor(native_devices[i], 0, &config);
    if (res == 0) {
      res = usb_copy_config_descriptor(dst, config);
      libusb_free_config_descriptor(config);
      if (res < 0) {
        usb_free_device(dst);
        usb_free_device_list(list);
        libusb_free_device_list(native_devices, 1);
        return res;
      }
    }

    list->count++;
  }

  libusb_free_device_list(native_devices, 1);
  return 0;
}

void
usb_free_device_list(struct usb_device_list *list)
{
  if (list == NULL)
    return;

  for (size_t i = 0; i < list->count; i++) {
    usb_free_device(&list->devices[i]);
  }
  free(list->devices);
  list->devices = NULL;
  list->count = 0;
}

int
usb_get_bus_device_strings(const struct usb_device *device,
                           char *bus_buffer, size_t bus_buffer_size,
                           char *device_buffer, size_t device_buffer_size)
{
  if (device == NULL || bus_buffer == NULL || device_buffer == NULL ||
      bus_buffer_size == 0 || device_buffer_size == 0) {
    return LIBUSB_ERROR_INVALID_PARAM;
  }

  if (snprintf(bus_buffer, bus_buffer_size, "%03u", device->bus_number) >=
      (int)bus_buffer_size) {
    return LIBUSB_ERROR_OVERFLOW;
  }

  if (snprintf(device_buffer, device_buffer_size, "%03u",
               device->device_address) >= (int)device_buffer_size) {
    return LIBUSB_ERROR_OVERFLOW;
  }

  return 0;
}

bool
usb_device_get_bulk_endpoints(const struct usb_device *device,
                              struct usb_bulk_endpoints *endpoints)
{
  if (device == NULL || endpoints == NULL)
    return false;

  memset(endpoints, 0, sizeof(*endpoints));
  for (size_t i = 0; i < device->interface_count; i++) {
    const struct usb_interface_descriptor *interface_desc =
        &device->interfaces[i];
    bool found_in = false;
    bool found_out = false;

    endpoints->interface_number = interface_desc->number;
    endpoints->alternate_setting = interface_desc->alternate_setting;

    for (size_t j = 0; j < interface_desc->endpoint_count; j++) {
      const struct usb_endpoint_descriptor *endpoint =
          &interface_desc->endpoints[j];
      if ((endpoint->attributes & USB_ENDPOINT_TYPE_MASK) !=
          USB_ENDPOINT_TYPE_BULK) {
        continue;
      }

      if ((endpoint->address & USB_ENDPOINT_DIR_MASK) == USB_ENDPOINT_IN) {
        endpoints->endpoint_in = endpoint->address;
        if (endpoint->max_packet_size > endpoints->max_packet_size)
          endpoints->max_packet_size = endpoint->max_packet_size;
        found_in = true;
      } else if ((endpoint->address & USB_ENDPOINT_DIR_MASK) ==
                 USB_ENDPOINT_OUT) {
        endpoints->endpoint_out = endpoint->address;
        if (endpoint->max_packet_size > endpoints->max_packet_size)
          endpoints->max_packet_size = endpoint->max_packet_size;
        found_out = true;
      }
    }

    if (found_in && found_out)
      return true;
  }

  return false;
}

int
usb_open(const struct usb_device *device, usb_dev_handle **handle)
{
  if (device == NULL || handle == NULL || device->native_device == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  *handle = NULL;
  usb_dev_handle *wrapper = calloc(1, sizeof(*wrapper));
  if (wrapper == NULL)
    return LIBUSB_ERROR_NO_MEM;

  int res = libusb_open((libusb_device *)device->native_device,
                        &wrapper->handle);
  if (res < 0) {
    free(wrapper);
    return res;
  }

  *handle = wrapper;
  return 0;
}

int
usb_close(usb_dev_handle *handle)
{
  if (handle == NULL)
    return 0;

  if (handle->handle != NULL)
    libusb_close(handle->handle);
  free(handle);
  return 0;
}

int
usb_set_configuration(usb_dev_handle *handle, int configuration_value)
{
  if (handle == NULL || handle->handle == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  int current_configuration = -1;
  if (libusb_get_configuration(handle->handle, &current_configuration) == 0 &&
      current_configuration == configuration_value) {
    return 0;
  }

  return libusb_set_configuration(handle->handle, configuration_value);
}

int
usb_claim_interface(usb_dev_handle *handle, int interface_number)
{
  if (handle == NULL || handle->handle == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  return libusb_claim_interface(handle->handle, interface_number);
}

int
usb_release_interface(usb_dev_handle *handle, int interface_number)
{
  if (handle == NULL || handle->handle == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  return libusb_release_interface(handle->handle, interface_number);
}

int
usb_set_altinterface(usb_dev_handle *handle, int interface_number,
                     int alternate_setting)
{
  if (handle == NULL || handle->handle == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  return libusb_set_interface_alt_setting(handle->handle, interface_number,
                                          alternate_setting);
}

int
usb_reset(usb_dev_handle *handle)
{
  if (handle == NULL || handle->handle == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;

  return libusb_reset_device(handle->handle);
}

static int
usb_bulk_transfer(usb_dev_handle *handle, unsigned char endpoint,
                  unsigned char *data, size_t size, int timeout)
{
  if (handle == NULL || handle->handle == NULL)
    return LIBUSB_ERROR_INVALID_PARAM;
  if (size > (size_t)INT_MAX)
    return LIBUSB_ERROR_INVALID_PARAM;

  int transferred = 0;
  int res = libusb_bulk_transfer(handle->handle, endpoint, data, (int)size,
                                 &transferred, timeout);
  if (res < 0)
    return res;

  return transferred;
}

int
usb_bulk_read(usb_dev_handle *handle, unsigned char endpoint,
              unsigned char *data, size_t size, int timeout)
{
  return usb_bulk_transfer(handle, endpoint, data, size, timeout);
}

int
usb_bulk_write(usb_dev_handle *handle, unsigned char endpoint,
               const unsigned char *data, size_t size, int timeout)
{
  return usb_bulk_transfer(handle, endpoint, (unsigned char *)data, size,
                           timeout);
}

int
usb_get_string_simple(usb_dev_handle *handle, int string_index,
                      char *buffer, size_t buffer_size)
{
  if (buffer == NULL || buffer_size == 0)
    return LIBUSB_ERROR_INVALID_PARAM;

  buffer[0] = '\0';
  if (handle == NULL || handle->handle == NULL || string_index <= 0)
    return 0;

  int res = libusb_get_string_descriptor_ascii(
      handle->handle, (uint8_t)string_index, (unsigned char *)buffer,
      (int)(buffer_size - 1));
  if (res < 0)
    return res;

  buffer[res] = '\0';
  return res;
}

const char *
usb_strerror(int result)
{
  return libusb_strerror((enum libusb_error)result);
}

bool
usb_error_is_timeout(int result)
{
  return result == LIBUSB_ERROR_TIMEOUT;
}

bool
usb_error_is_access(int result)
{
  return result == LIBUSB_ERROR_ACCESS;
}
