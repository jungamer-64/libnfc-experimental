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
 * Copyright (C) 2020      Adam Laurie
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
 * @file nfc-internal.c
 * @brief Provide some useful internal functions
 */

#include <nfc/nfc.h>
#include "nfc-internal.h"
#include "nfc-secure.h"

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif

#include <string.h>

void string_as_boolean(const char *s, bool *value)
{
  if (s)
  {
    if (!(*value))
    {
      if ((strcmp(s, "yes") == 0) ||
          (strcmp(s, "true") == 0) ||
          (strcmp(s, "1") == 0))
      {
        *value = true;
        return;
      }
    }
    else
    {
      if ((strcmp(s, "no") == 0) ||
          (strcmp(s, "false") == 0) ||
          (strcmp(s, "0") == 0))
      {
        *value = false;
        return;
      }
    }
  }
}

void
nfc_rs_context_log_init(const nfc_context *context)
{
  log_init(context);
}

void
nfc_rs_context_log_exit(void)
{
  log_exit();
}

void
nfc_rs_log_message(uint8_t group, const char *category, uint8_t priority, const char *message)
{
  log_put_message(group, category, priority, message);
}

void prepare_initiator_data(const nfc_modulation nm, uint8_t **ppbtInitiatorData, size_t *pszInitiatorData)
{
  switch (nm.nmt)
  {
  case NMT_ISO14443B:
    // Application Family Identifier (AFI) must equals 0x00 in order to wakeup all ISO14443-B PICCs (see ISO/IEC 14443-3)
    *ppbtInitiatorData = (uint8_t *)"\x00";
    *pszInitiatorData = 1;
    break;
  case NMT_ISO14443BI:
    // APGEN
    *ppbtInitiatorData = (uint8_t *)"\x01\x0b\x3f\x80";
    *pszInitiatorData = 4;
    break;
  case NMT_FELICA:
    // polling payload must be present (see ISO/IEC 18092 11.2.2.5)
    *ppbtInitiatorData = (uint8_t *)"\x00\xff\xff\x01\x00";
    *pszInitiatorData = 5;
    break;
  case NMT_ISO14443A:
  case NMT_ISO14443B2CT:
  case NMT_ISO14443B2SR:
  case NMT_ISO14443BICLASS:
  case NMT_JEWEL:
  case NMT_BARCODE:
  case NMT_DEP:
    *ppbtInitiatorData = NULL;
    *pszInitiatorData = 0;
    break;
  }
}
