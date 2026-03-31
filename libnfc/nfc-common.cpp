/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Copyright (C) 2025-2026 jungamer-64
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
 * @file nfc-common.cpp
 * @brief Implementation of common utility functions
 *
 * Refactored for improved type safety and error handling:
 * - Enhanced const correctness throughout
 * - More robust error code propagation
 * - Better input validation
 * - Reduced magic numbers
 * - Improved documentation
 */

#include "nfc-common.h"
extern "C"
{
#include "chips/pn53x.h"
}
/* =========================================================================
 * DEVICE INITIALIZATION ERROR HANDLING
 * ========================================================================= */

/**
 * @brief Comprehensive device initialization error handler
 */
int nfc_device_init_failed(nfc_device *pnd,
                           nfc_port_handle_t port,
                           nfc_port_close_fn close_fn,
                           void **ports,
                           bool chip_data_allocated)
{
  /* Close communication port if provided */
  if (port != NULL && close_fn != NULL)
  {
    close_fn(port);
  }

  /* Free chip-specific data if it was allocated */
  if (pnd != NULL && chip_data_allocated)
  {
    pn53x_data_free(pnd);
  }

  /* Free device structure */
  if (pnd != NULL)
  {
    nfc_device_free(pnd);
  }

  /* Clean up port array */
  nfc_free_array(ports);

  return 0; /* Return value for scan functions */
}

/**
 * @brief Common resource cleanup for device open failures
 */
void nfc_device_open_failed(nfc_device *pnd,
                            void *driver_data,
                            bool chip_data_allocated)
{
  if (pnd == NULL)
  {
    /* Device structure not allocated, free driver_data directly */
    free(driver_data);
    return;
  }

  /* Free chip-specific data if allocated */
  if (chip_data_allocated)
  {
    pn53x_data_free(pnd);
  }

  /* nfc_device_free will handle driver_data cleanup */
  nfc_device_free(pnd);
}
