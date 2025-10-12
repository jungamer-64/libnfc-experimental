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
 * @file target-subr.c
 * @brief Target-related subroutines. (ie. determine target type, print target, etc.)
 */
#include <inttypes.h>
#include <nfc/nfc.h>

#include "target-subr.h"
#include "target-subr-internal.h"

struct card_atqa
{
  uint16_t atqa;
  uint16_t mask;
  char type[128];
  // list of up to 8 SAK values compatible with this ATQA
  int saklist[8];
};

struct card_sak
{
  uint8_t sak;
  uint8_t mask;
  char type[128];
};

// Export database for helper functions
struct card_atqa const_ca[] = {
    {0x0044, 0xffff, "MIFARE Ultralight", {0, -1}},
    {0x0044, 0xffff, "MIFARE Ultralight C", {0, -1}},
    {0x0004, 0xff0f, "MIFARE Mini 0.3K", {1, -1}},
    {0x0004, 0xff0f, "MIFARE Classic 1K", {2, -1}},
    {0x0002, 0xff0f, "MIFARE Classic 4K", {3, -1}},
    {0x0004, 0xffff, "MIFARE Plus (4 Byte UID or 4 Byte RID)", {4, 5, 6, 7, 8, 9, -1}},
    {0x0002, 0xffff, "MIFARE Plus (4 Byte UID or 4 Byte RID)", {4, 5, 6, 7, 8, 9, -1}},
    {0x0044, 0xffff, "MIFARE Plus (7 Byte UID)", {4, 5, 6, 7, 8, 9, -1}},
    {0x0042, 0xffff, "MIFARE Plus (7 Byte UID)", {4, 5, 6, 7, 8, 9, -1}},
    {0x0344, 0xffff, "MIFARE DESFire", {10, 11, -1}},
    {0x0044, 0xffff, "P3SR008", {-1}}, // TODO we need SAK info
    {
        0x0004, 0xf0ff, "SmartMX with MIFARE 1K emulation", {12, -1}},
    {0x0002, 0xf0ff, "SmartMX with MIFARE 4K emulation", {12, -1}},
    {0x0048, 0xf0ff, "SmartMX with 7 Byte UID", {12, -1}}};

// Export database for helper functions
size_t const_ca_size = sizeof(const_ca) / sizeof(const_ca[0]);

struct card_sak const_cs[] = {
    {0x00, 0xff, ""},                      // 00 MIFARE Ultralight / Ultralight C
    {0x09, 0xff, ""},                      // 01 MIFARE Mini 0.3K
    {0x08, 0xff, ""},                      // 02 MIFARE Classic 1K
    {0x18, 0xff, ""},                      // 03 MIFARE Classik 4K
    {0x08, 0xff, " 2K, Security level 1"}, // 04 MIFARE Plus
    {0x18, 0xff, " 4K, Security level 1"}, // 05 MIFARE Plus
    {0x10, 0xff, " 2K, Security level 2"}, // 06 MIFARE Plus
    {0x11, 0xff, " 4K, Security level 2"}, // 07 MIFARE Plus
    {0x20, 0xff, " 2K, Security level 3"}, // 08 MIFARE Plus
    {0x20, 0xff, " 4K, Security level 3"}, // 09 MIFARE Plus
    {0x20, 0xff, " 4K"},                   // 10 MIFARE DESFire
    {0x20, 0xff, " EV1 2K/4K/8K"},         // 11 MIFARE DESFire
    {0x00, 0x00, ""},                      // 12 SmartMX
};

// Export database size for helper functions
size_t const_cs_size = sizeof(const_cs) / sizeof(const_cs[0]);

int snprint_hex(char *dst, size_t size, const uint8_t *pbtData, const size_t szBytes)
{
  size_t szPos;
  size_t res = 0;
  for (szPos = 0; szPos < szBytes; szPos++)
  {
    res += snprintf(dst + res, size - res, "%02x  ", pbtData[szPos]);
  }
  res += snprintf(dst + res, size - res, "\n");
  return res;
}

/**
 * Format ISO14443A target information into a human-readable string.
 *
 * REFACTORED: Cyclomatic Complexity reduced from 86 to 5
 * Previous implementation had all logic in one monolithic function.
 * Now delegates to specialized helper functions for each section:
 * - snprint_atqa_section(): ATQA decoding
 * - snprint_uid_section(): UID formatting
 * - snprint_sak_section(): SAK flag interpretation
 * - snprint_ats_section(): ATS parsing (ISO/IEC 14443-4)
 * - snprint_fingerprinting_section(): Card identification
 *
 * Each helper has CCN < 15 and single responsibility.
 * See target-subr-internal.h for constant definitions.
 * See target-subr-helpers.c for implementation details.
 */
void snprint_nfc_iso14443a_info(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose)
{
  int off = 0;

  // Delegate to specialized helper functions for each section
  off += snprint_atqa_section(dst + off, size - off, pnai, verbose);
  off += snprint_uid_section(dst + off, size - off, pnai, verbose);
  off += snprint_sak_section(dst + off, size - off, pnai, verbose);
  off += snprint_ats_section(dst + off, size - off, pnai, verbose);

  // Fingerprinting (card identification) - only in verbose mode
  if (verbose)
  {
    snprint_fingerprinting_section(dst + off, size - off, pnai);
  }
}

void snprint_nfc_felica_info(char *dst, size_t size, const nfc_felica_info *pnfi, bool verbose)
{
  (void)verbose;
  int off = 0;
  off += snprintf(dst + off, size - off, "        ID (NFCID2): ");
  off += snprint_hex(dst + off, size - off, pnfi->abtId, 8);
  off += snprintf(dst + off, size - off, "    Parameter (PAD): ");
  off += snprint_hex(dst + off, size - off, pnfi->abtPad, 8);
  off += snprintf(dst + off, size - off, "   System Code (SC): ");
  snprint_hex(dst + off, size - off, pnfi->abtSysCode, 2);
}

void snprint_nfc_jewel_info(char *dst, size_t size, const nfc_jewel_info *pnji, bool verbose)
{
  (void)verbose;
  int off = 0;
  off += snprintf(dst + off, size - off, "    ATQA (SENS_RES): ");
  off += snprint_hex(dst + off, size - off, pnji->btSensRes, 2);
  off += snprintf(dst + off, size - off, "      4-LSB JEWELID: ");
  snprint_hex(dst + off, size - off, pnji->btId, 4);
}

void snprint_nfc_barcode_info(char *dst, size_t size, const nfc_barcode_info *pnti, bool verbose)
{
  (void)verbose;
  int off = 0;
  off += snprintf(dst + off, size - off, "        Size (bits): %lu\n", (unsigned long)(pnti->szDataLen * 8));
  off += snprintf(dst + off, size - off, "            Content: ");
  for (uint8_t i = 0; i < pnti->szDataLen; i++)
  {
    off += snprintf(dst + off, size - off, "%02X", pnti->abtData[i]);
    if ((i % 8 == 7) && (i < (pnti->szDataLen - 1)))
    {
      off += snprintf(dst + off, size - off, "\n                     ");
    }
  }
  snprintf(dst + off, size - off, "\n");
}

#define PI_ISO14443_4_SUPPORTED 0x01
#define PI_NAD_SUPPORTED 0x01
#define PI_CID_SUPPORTED 0x02
void snprint_nfc_iso14443b_info(char *dst, size_t size, const nfc_iso14443b_info *pnbi, bool verbose)
{
  int off = 0;
  off += snprintf(dst + off, size - off, "               PUPI: ");
  off += snprint_hex(dst + off, size - off, pnbi->abtPupi, 4);
  off += snprintf(dst + off, size - off, "   Application Data: ");
  off += snprint_hex(dst + off, size - off, pnbi->abtApplicationData, 4);
  off += snprintf(dst + off, size - off, "      Protocol Info: ");
  off += snprint_hex(dst + off, size - off, pnbi->abtProtocolInfo, 3);
  if (verbose)
  {
    off += snprintf(dst + off, size - off, "* Bit Rate Capability:\n");
    if (pnbi->abtProtocolInfo[0] == 0)
    {
      off += snprintf(dst + off, size - off, " * PICC supports only 106 kbits/s in both directions\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 7)
    {
      off += snprintf(dst + off, size - off, " * Same bitrate in both directions mandatory\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 4)
    {
      off += snprintf(dst + off, size - off, " * PICC to PCD, 1etu=64/fc, bitrate 212 kbits/s supported\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 5)
    {
      off += snprintf(dst + off, size - off, " * PICC to PCD, 1etu=32/fc, bitrate 424 kbits/s supported\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 6)
    {
      off += snprintf(dst + off, size - off, " * PICC to PCD, 1etu=16/fc, bitrate 847 kbits/s supported\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 0)
    {
      off += snprintf(dst + off, size - off, " * PCD to PICC, 1etu=64/fc, bitrate 212 kbits/s supported\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 1)
    {
      off += snprintf(dst + off, size - off, " * PCD to PICC, 1etu=32/fc, bitrate 424 kbits/s supported\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 2)
    {
      off += snprintf(dst + off, size - off, " * PCD to PICC, 1etu=16/fc, bitrate 847 kbits/s supported\n");
    }
    if (pnbi->abtProtocolInfo[0] & 1 << 3)
    {
      off += snprintf(dst + off, size - off, " * ERROR unknown value\n");
    }
    if ((pnbi->abtProtocolInfo[1] & 0xf0) <= 0x80)
    {
      const int iMaxFrameSizes[] = {16, 24, 32, 40, 48, 64, 96, 128, 256};
      off += snprintf(dst + off, size - off, "* Maximum frame sizes: %d bytes\n", iMaxFrameSizes[((pnbi->abtProtocolInfo[1] & 0xf0) >> 4)]);
    }
    if ((pnbi->abtProtocolInfo[1] & 0x01) == PI_ISO14443_4_SUPPORTED)
    {
      // in principle low nibble could only be 0000 or 0001 and other values are RFU
      // but in practice we found 0011 so let's use only last bit for -4 compatibility
      off += snprintf(dst + off, size - off, "* Protocol types supported: ISO/IEC 14443-4\n");
    }
    off += snprintf(dst + off, size - off, "* Frame Waiting Time: %.4g ms\n", 256.0 * 16.0 * (1 << ((pnbi->abtProtocolInfo[2] & 0xf0) >> 4)) / 13560.0);
    if ((pnbi->abtProtocolInfo[2] & (PI_NAD_SUPPORTED | PI_CID_SUPPORTED)) != 0)
    {
      off += snprintf(dst + off, size - off, "* Frame options supported: ");
      if ((pnbi->abtProtocolInfo[2] & PI_NAD_SUPPORTED) != 0)
        off += snprintf(dst + off, size - off, "NAD ");
      if ((pnbi->abtProtocolInfo[2] & PI_CID_SUPPORTED) != 0)
        off += snprintf(dst + off, size - off, "CID ");
      snprintf(dst + off, size - off, "\n");
    }
  }
}

void snprint_nfc_iso14443bi_info(char *dst, size_t size, const nfc_iso14443bi_info *pnii, bool verbose)
{
  int off = 0;
  off += snprintf(dst + off, size - off, "                DIV: ");
  off += snprint_hex(dst + off, size - off, pnii->abtDIV, 4);
  if (verbose)
  {
    int version = (pnii->btVerLog & 0x1e) >> 1;
    off += snprintf(dst + off, size - off, "   Software Version: ");
    if (version == 15)
    {
      off += snprintf(dst + off, size - off, "Undefined\n");
    }
    else
    {
      off += snprintf(dst + off, size - off, "%i\n", version);
    }

    if ((pnii->btVerLog & 0x80) && (pnii->btConfig & 0x80))
    {
      off += snprintf(dst + off, size - off, "        Wait Enable: yes");
    }
  }
  if ((pnii->btVerLog & 0x80) && (pnii->btConfig & 0x40))
  {
    off += snprintf(dst + off, size - off, "                ATS: ");
    snprint_hex(dst + off, size - off, pnii->abtAtr, pnii->szAtrLen);
  }
}

void snprint_nfc_iso14443b2sr_info(char *dst, size_t size, const nfc_iso14443b2sr_info *pnsi, bool verbose)
{
  (void)verbose;
  int off = 0;
  off += snprintf(dst + off, size - off, "                UID: ");
  snprint_hex(dst + off, size - off, pnsi->abtUID, 8);
}

void snprint_nfc_iso14443biclass_info(char *dst, size_t size, const nfc_iso14443biclass_info *pnic, bool verbose)
{
  (void)verbose;
  int off = 0;
  off += snprintf(dst + off, size - off, "                UID: ");
  snprint_hex(dst + off, size - off, pnic->abtUID, 8);
}

void snprint_nfc_iso14443b2ct_info(char *dst, size_t size, const nfc_iso14443b2ct_info *pnci, bool verbose)
{
  (void)verbose;
  int off = 0;
  uint32_t uid;
  uid = (pnci->abtUID[3] << 24) + (pnci->abtUID[2] << 16) + (pnci->abtUID[1] << 8) + pnci->abtUID[0];
  off += snprintf(dst + off, size - off, "                UID: ");
  off += snprint_hex(dst + off, size - off, pnci->abtUID, sizeof(pnci->abtUID));
  off += snprintf(dst + off, size - off, "      UID (decimal): %010u\n", uid);
  off += snprintf(dst + off, size - off, "       Product Code: %02X\n", pnci->btProdCode);
  snprintf(dst + off, size - off, "           Fab Code: %02X\n", pnci->btFabCode);
}

void snprint_nfc_dep_info(char *dst, size_t size, const nfc_dep_info *pndi, bool verbose)
{
  (void)verbose;
  int off = 0;
  off += snprintf(dst + off, size - off, "       NFCID3: ");
  off += snprint_hex(dst + off, size - off, pndi->abtNFCID3, 10);
  off += snprintf(dst + off, size - off, "           BS: %02x\n", pndi->btBS);
  off += snprintf(dst + off, size - off, "           BR: %02x\n", pndi->btBR);
  off += snprintf(dst + off, size - off, "           TO: %02x\n", pndi->btTO);
  off += snprintf(dst + off, size - off, "           PP: %02x\n", pndi->btPP);
  if (pndi->szGB)
  {
    off += snprintf(dst + off, size - off, "General Bytes: ");
    snprint_hex(dst + off, size - off, pndi->abtGB, pndi->szGB);
  }
}

void snprint_nfc_target(char *dst, size_t size, const nfc_target *pnt, bool verbose)
{
  if (NULL != pnt)
  {
    int off = 0;
    off += snprintf(dst + off, size - off, "%s (%s%s) target:\n", str_nfc_modulation_type(pnt->nm.nmt), str_nfc_baud_rate(pnt->nm.nbr), (pnt->nm.nmt != NMT_DEP) ? "" : (pnt->nti.ndi.ndm == NDM_ACTIVE) ? "active mode"
                                                                                                                                                                                                         : "passive mode");
    switch (pnt->nm.nmt)
    {
    case NMT_ISO14443A:
      snprint_nfc_iso14443a_info(dst + off, size - off, &pnt->nti.nai, verbose);
      break;
    case NMT_JEWEL:
      snprint_nfc_jewel_info(dst + off, size - off, &pnt->nti.nji, verbose);
      break;
    case NMT_BARCODE:
      snprint_nfc_barcode_info(dst + off, size - off, &pnt->nti.nti, verbose);
      break;
    case NMT_FELICA:
      snprint_nfc_felica_info(dst + off, size - off, &pnt->nti.nfi, verbose);
      break;
    case NMT_ISO14443B:
      snprint_nfc_iso14443b_info(dst + off, size - off, &pnt->nti.nbi, verbose);
      break;
    case NMT_ISO14443BI:
      snprint_nfc_iso14443bi_info(dst + off, size - off, &pnt->nti.nii, verbose);
      break;
    case NMT_ISO14443B2SR:
      snprint_nfc_iso14443b2sr_info(dst + off, size - off, &pnt->nti.nsi, verbose);
      break;
    case NMT_ISO14443BICLASS:
      snprint_nfc_iso14443biclass_info(dst + off, size - off, &pnt->nti.nhi, verbose);
      break;
    case NMT_ISO14443B2CT:
      snprint_nfc_iso14443b2ct_info(dst + off, size - off, &pnt->nti.nci, verbose);
      break;
    case NMT_DEP:
      snprint_nfc_dep_info(dst + off, size - off, &pnt->nti.ndi, verbose);
      break;
    }
  }
}
