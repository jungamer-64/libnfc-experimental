#include <stdlib.h>
#include <string.h>

#include "buses/usbbus.h"

static void
expect_true(int condition)
{
  if (!condition)
    abort();
}

static void
test_bus_device_formatting(void)
{
  struct usb_device device = {
      .bus_number = 4,
      .device_address = 23,
  };
  char bus[4];
  char addr[4];

  expect_true(usb_get_bus_device_strings(&device, bus, sizeof(bus), addr,
                                         sizeof(addr)) == 0);
  expect_true(strcmp(bus, "004") == 0);
  expect_true(strcmp(addr, "023") == 0);
}

static void
test_bulk_endpoint_discovery(void)
{
  struct usb_endpoint_descriptor endpoints[] = {
      {.address = 0x83, .attributes = 0x03, .max_packet_size = 0x10},
      {.address = 0x81, .attributes = 0x02, .max_packet_size = 0x40},
      {.address = 0x02, .attributes = 0x02, .max_packet_size = 0x20},
  };
  struct usb_interface_descriptor interfaces[] = {
      {.number = 0, .alternate_setting = 0, .endpoint_count = 3,
       .endpoints = endpoints},
  };
  struct usb_device device = {
      .interface_count = 1,
      .interfaces = interfaces,
  };
  struct usb_bulk_endpoints bulk_endpoints;

  expect_true(usb_device_get_bulk_endpoints(&device, &bulk_endpoints));
  expect_true(bulk_endpoints.interface_number == 0);
  expect_true(bulk_endpoints.alternate_setting == 0);
  expect_true(bulk_endpoints.endpoint_in == 0x81);
  expect_true(bulk_endpoints.endpoint_out == 0x02);
  expect_true(bulk_endpoints.max_packet_size == 0x40);
}

int
main(void)
{
  test_bus_device_formatting();
  test_bulk_endpoint_discovery();
  return 0;
}
