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
