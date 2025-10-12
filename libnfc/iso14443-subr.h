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
 */

/**
 * @file iso14443-subr.h
 * @brief ISO14443 subroutines
 */

#ifndef __NFC_ISO14443_SUBR_H__
#define __NFC_ISO14443_SUBR_H__

#include <stddef.h>
#include <stdint.h>

void iso14443a_crc(const uint8_t *pbtData, size_t szLen, uint8_t *pbtCrc);
void iso14443a_crc_append(uint8_t *pbtData, size_t szLen);
void iso14443b_crc_append(uint8_t *pbtData, size_t szLen);
uint8_t *iso14443a_locate_historical_bytes(const uint8_t *pbtAts, size_t szAts, size_t *pszTk);
void iso14443_cascade_uid(const uint8_t *abtUID, const size_t szUID, uint8_t *pbtCascadedUID, size_t *pszCascadedUID);

#endif // __NFC_ISO14443_SUBR_H__
