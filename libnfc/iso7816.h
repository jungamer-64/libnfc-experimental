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
 * @file iso7816.h
 * @brief ISO/IEC 7816 constants and definitions
 */

#ifndef __NFC_ISO7816_H__
#define __NFC_ISO7816_H__

// ISO 7816-3 / 7816-4 APDU length constants
// Short APDU format (as per ISO 7816-3)
#define ISO7816_SHORT_C_APDU_MAX_LEN   261  /* CLA + INS + P1 + P2 + Lc (1 byte) + Data (255 bytes) + Le (1 byte) */
#define ISO7816_SHORT_R_APDU_MAX_LEN   258  /* Data (256 bytes) + SW1 + SW2 */

// Extended APDU format (as per ISO 7816-3 amendment 1)
#define ISO7816_EXT_C_APDU_MAX_LEN     65544 /* CLA + INS + P1 + P2 + Lc (3 bytes) + Data (65535 bytes) + Le (2 bytes) */
#define ISO7816_EXT_R_APDU_MAX_LEN     65538 /* Data (65536 bytes) + SW1 + SW2 */

#endif /* __NFC_ISO7816_H__ */
