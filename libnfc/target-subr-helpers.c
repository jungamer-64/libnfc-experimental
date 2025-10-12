/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 */

/**
 * @file target-subr-helpers.c
 * @brief Helper functions for ISO14443A target information formatting
 *
 * This file contains helper functions extracted from snprint_nfc_iso14443a_info()
 * to reduce its cyclomatic complexity from 86 to <15.
 * Each function has a single responsibility and CCN <15.
 */

#include <stdio.h>
#include <inttypes.h>
#include "target-subr-internal.h"
#include "target-subr.h"

/**
 * Lookup table for maximum frame sizes according to FSCI value
 * Index corresponds to FSCI (0-8), value is frame size in bytes
 * Source: ISO/IEC 14443-4
 */
static const int max_frame_sizes[] = {16, 24, 32, 40, 48, 64, 96, 128, 256};

/**
 * Get maximum frame size from FSCI value
 *
 * @param fsci Frame Size for proximity Card Integer (0-8)
 * @return Maximum frame size in bytes, or 16 if out of range
 */
static inline int get_max_frame_size(uint8_t fsci)
{
  if (fsci < sizeof(max_frame_sizes) / sizeof(max_frame_sizes[0]))
  {
    return max_frame_sizes[fsci];
  }
  return 16; // Default minimum
}

/**
 * Calculate Frame Waiting Time from FWI value
 *
 * @param fwi Frame Waiting Integer (0-15)
 * @return Frame Waiting Time in milliseconds
 */
static inline double calculate_fwt_ms(uint8_t fwi)
{
  return (TIMING_FACTOR * (1 << fwi)) / FC_HZ * 1000.0;
}

/**
 * Print ATQA (Answer To Request Type A) section
 * Cyclomatic Complexity: 6
 */
int snprint_atqa_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose)
{
  int off = 0;

  off += snprintf(dst + off, size - off, "    ATQA (SENS_RES): ");
  off += snprint_hex(dst + off, size - off, pnai->abtAtqa, 2);

  if (!verbose)
  {
    return off;
  }

  // Decode UID size
  off += snprintf(dst + off, size - off, "* UID size: ");
  uint8_t uid_size_code = (pnai->abtAtqa[1] & ATQA_UID_SIZE_MASK) >> ATQA_UID_SIZE_SHIFT;

  switch (uid_size_code)
  {
  case ATQA_UID_SIZE_SINGLE:
    off += snprintf(dst + off, size - off, "single\n");
    break;
  case ATQA_UID_SIZE_DOUBLE:
    off += snprintf(dst + off, size - off, "double\n");
    break;
  case ATQA_UID_SIZE_TRIPLE:
    off += snprintf(dst + off, size - off, "triple\n");
    break;
  case ATQA_UID_SIZE_RFU:
    off += snprintf(dst + off, size - off, "RFU\n");
    break;
  }

  // Decode bit frame anticollision support
  off += snprintf(dst + off, size - off, "* bit frame anticollision ");
  uint8_t anticol_bits = pnai->abtAtqa[1] & ATQA_BITFRAME_ANTICOL_MASK;

  // Valid anticollision values: 0x01, 0x02, 0x04, 0x08, 0x10 (powers of 2)
  if (anticol_bits != 0 && (anticol_bits & (anticol_bits - 1)) == 0 && anticol_bits <= 0x10)
  {
    off += snprintf(dst + off, size - off, "supported\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "not supported\n");
  }

  return off;
}

/**
 * Print UID (Unique Identifier) section
 * Cyclomatic Complexity: 3
 */
int snprint_uid_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose)
{
  int off = 0;

  // Determine NFCID type (1 or 3)
  char nfcid_type = (pnai->abtUid[0] == UID_RANDOM_ID) ? '3' : '1';

  off += snprintf(dst + off, size - off, "       UID (NFCID%c): ", nfcid_type);
  off += snprint_hex(dst + off, size - off, pnai->abtUid, pnai->szUidLen);

  if (verbose && pnai->abtUid[0] == UID_RANDOM_ID)
  {
    off += snprintf(dst + off, size - off, "* Random UID\n");
  }

  return off;
}

/**
 * Print SAK (Select Acknowledge) section
 * Cyclomatic Complexity: 5
 */
int snprint_sak_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose)
{
  int off = 0;

  off += snprintf(dst + off, size - off, "      SAK (SEL_RES): ");
  off += snprint_hex(dst + off, size - off, &pnai->btSak, 1);

  if (!verbose)
  {
    return off;
  }

  // Check cascade bit
  if (pnai->btSak & SAK_UID_NOT_COMPLETE)
  {
    off += snprintf(dst + off, size - off, "* Warning! Cascade bit set: UID not complete\n");
  }

  // Check ISO/IEC 14443-4 compliance
  if (pnai->btSak & SAK_ISO14443_4_COMPLIANT)
  {
    off += snprintf(dst + off, size - off, "* Compliant with ISO/IEC 14443-4\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "* Not compliant with ISO/IEC 14443-4\n");
  }

  // Check ISO/IEC 18092 compliance
  if (pnai->btSak & SAK_ISO18092_COMPLIANT)
  {
    off += snprintf(dst + off, size - off, "* Compliant with ISO/IEC 18092\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "* Not compliant with ISO/IEC 18092\n");
  }

  return off;
}

/**
 * Print bitrate capability information from TA(1)
 * Cyclomatic Complexity: 9
 */
int snprint_ats_bitrate_capability(char *dst, size_t size, uint8_t TA)
{
  int off = 0;

  off += snprintf(dst + off, size - off, "* Bit Rate Capability:\n");

  if (TA == 0)
  {
    return off + snprintf(dst + off, size - off,
                          "  * PICC supports only 106 kbits/s in both directions\n");
  }

  if (TA & ATS_TA1_SAME_BITRATE)
  {
    off += snprintf(dst + off, size - off, "  * Same bitrate in both directions mandatory\n");
  }

  // PICC to PCD bitrates
  if (TA & ATS_TA1_DS2_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "  * PICC to PCD, DS=2, bitrate 212 kbits/s supported\n");
  }
  if (TA & ATS_TA1_DS4_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "  * PICC to PCD, DS=4, bitrate 424 kbits/s supported\n");
  }
  if (TA & ATS_TA1_DS8_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "  * PICC to PCD, DS=8, bitrate 847 kbits/s supported\n");
  }

  // PCD to PICC bitrates
  if (TA & ATS_TA1_DR2_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "  * PCD to PICC, DR=2, bitrate 212 kbits/s supported\n");
  }
  if (TA & ATS_TA1_DR4_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "  * PCD to PICC, DR=4, bitrate 424 kbits/s supported\n");
  }
  if (TA & ATS_TA1_DR8_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "  * PCD to PICC, DR=8, bitrate 847 kbits/s supported\n");
  }

  if (TA & ATS_TA1_ERROR_BIT)
  {
    off += snprintf(dst + off, size - off, "  * ERROR unknown value\n");
  }

  return off;
}

/**
 * Print frame timing information from TB(1)
 * Cyclomatic Complexity: 3
 */
int snprint_ats_frame_timing(char *dst, size_t size, uint8_t TB)
{
  int off = 0;

  uint8_t fwi = (TB & ATS_TB1_FWI_MASK) >> ATS_TB1_FWI_SHIFT;
  uint8_t sfgi = TB & ATS_TB1_SFGI_MASK;

  off += snprintf(dst + off, size - off, "* Frame Waiting Time: %.4g ms\n",
                  calculate_fwt_ms(fwi));

  if (sfgi == 0)
  {
    off += snprintf(dst + off, size - off, "* No Start-up Frame Guard Time required\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "* Start-up Frame Guard Time: %.4g ms\n",
                    calculate_fwt_ms(sfgi));
  }

  return off;
}

/**
 * Print node address and CID support from TC(1)
 * Cyclomatic Complexity: 3
 */
int snprint_ats_node_cid_support(char *dst, size_t size, uint8_t TC)
{
  int off = 0;

  if (TC & ATS_TC1_NAD_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "* Node Address supported\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "* Node Address not supported\n");
  }

  if (TC & ATS_TC1_CID_SUPPORTED)
  {
    off += snprintf(dst + off, size - off, "* Card IDentifier supported\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "* Card IDentifier not supported\n");
  }

  return off;
}

/**
 * Print ATS (Answer To Select) section
 * Cyclomatic Complexity: 7
 */
int snprint_ats_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose)
{
  int off = 0;

  if (pnai->szAtsLen == 0)
  {
    return 0;
  }

  // Print raw ATS
  off += snprintf(dst + off, size - off, "                ATS: ");
  off += snprint_hex(dst + off, size - off, pnai->abtAts, pnai->szAtsLen);

  if (!verbose)
  {
    return off;
  }

  // Decode ATS according to ISO/IEC 14443-4 (5.2 Answer to select)
  uint8_t t0 = pnai->abtAts[0];
  uint8_t fsci = t0 & ATS_T0_FSCI_MASK;

  off += snprintf(dst + off, size - off,
                  "* Max Frame Size accepted by PICC: %d bytes\n",
                  get_max_frame_size(fsci));

  size_t offset = 1;

  // TA(1) - Bitrate capability
  if (t0 & ATS_T0_TA1_PRESENT)
  {
    off += snprint_ats_bitrate_capability(dst + off, size - off, pnai->abtAts[offset]);
    offset++;
  }

  // TB(1) - Frame timing
  if (t0 & ATS_T0_TB1_PRESENT)
  {
    off += snprint_ats_frame_timing(dst + off, size - off, pnai->abtAts[offset]);
    offset++;
  }

  // TC(1) - Node address and CID support
  if (t0 & ATS_T0_TC1_PRESENT)
  {
    off += snprint_ats_node_cid_support(dst + off, size - off, pnai->abtAts[offset]);
    offset++;
  }

  // Historical bytes
  if (pnai->szAtsLen > offset)
  {
    off += snprint_ats_historical_bytes(dst + off, size - off, pnai, offset);
  }

  return off;
}

// (To be continued in next file chunk - Mifare proprietary, COMPACT-TLV, and fingerprinting functions)
