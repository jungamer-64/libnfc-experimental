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
 *
 */

/**
 * @file rust_usb_bridge.h
 * @brief Internal Rust-backed USB helper declarations for USB NFC drivers
 */

#ifndef __NFC_RUST_USB_BRIDGE_H__
#  define __NFC_RUST_USB_BRIDGE_H__

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct usb_dev_handle usb_dev_handle;

struct usb_endpoint_descriptor {
  uint8_t address;
  uint8_t attributes;
  uint16_t max_packet_size;
};

struct usb_interface_descriptor {
  uint8_t number;
  uint8_t alternate_setting;
  size_t endpoint_count;
  struct usb_endpoint_descriptor *endpoints;
};

struct usb_device {
  void *native_device;
  uint16_t vendor_id;
  uint16_t product_id;
  uint8_t manufacturer_string_index;
  uint8_t product_string_index;
  uint8_t bus_number;
  uint8_t device_address;
  uint8_t configuration_value;
  size_t interface_count;
  struct usb_interface_descriptor *interfaces;
};

struct usb_device_list {
  struct usb_device *devices;
  size_t count;
};

struct usb_bulk_endpoints {
  uint8_t interface_number;
  int alternate_setting;
  uint8_t endpoint_in;
  uint8_t endpoint_out;
  uint16_t max_packet_size;
};

int usb_prepare(void);
int usb_get_device_list(struct usb_device_list *list);
void usb_free_device_list(struct usb_device_list *list);
int usb_get_bus_device_strings(const struct usb_device *device,
                               char *bus_buffer, size_t bus_buffer_size,
                               char *device_buffer, size_t device_buffer_size);
bool usb_device_get_bulk_endpoints(const struct usb_device *device,
                                   struct usb_bulk_endpoints *endpoints);
int usb_open(const struct usb_device *device, usb_dev_handle **handle);
int usb_close(usb_dev_handle *handle);
int usb_set_configuration(usb_dev_handle *handle, int configuration_value);
int usb_claim_interface(usb_dev_handle *handle, int interface_number);
int usb_release_interface(usb_dev_handle *handle, int interface_number);
int usb_set_altinterface(usb_dev_handle *handle, int interface_number,
                         int alternate_setting);
int usb_reset(usb_dev_handle *handle);
int usb_bulk_read(usb_dev_handle *handle, unsigned char endpoint,
                  unsigned char *data, size_t size, int timeout);
int usb_bulk_write(usb_dev_handle *handle, unsigned char endpoint,
                   const unsigned char *data, size_t size, int timeout);
int usb_get_string_simple(usb_dev_handle *handle, int string_index,
                          char *buffer, size_t buffer_size);
const char *usb_strerror(int result);
bool usb_error_is_timeout(int result);
bool usb_error_is_access(int result);

#define _usb_strerror(X) usb_strerror(X)

#endif // __NFC_RUST_USB_BRIDGE_H__
