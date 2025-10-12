/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Helper functions for ISO14443A target information formatting (Part 2)
 * Mifare proprietary format, COMPACT-TLV, and fingerprinting
 */

#include <stdio.h>
#include <string.h>
#include <inttypes.h>
#include "target-subr-internal.h"
#include "target-subr.h"

// External card database from target-subr.c
struct card_atqa
{
  uint16_t atqa;
  uint16_t mask;
  char type[128];
  int saklist[8];
};

struct card_sak
{
  uint8_t sak;
  uint8_t mask;
  char type[128];
};

extern struct card_atqa const_ca[];
extern struct card_sak const_cs[];
extern size_t const_ca_size;
extern size_t const_cs_size;

/**
 * Print Mifare chip type
 * Cyclomatic Complexity: 4
 */
static int snprint_mifare_chip_type(char *dst, size_t size, uint8_t chip_type_code)
{
  switch (chip_type_code & MIFARE_CTC_CHIP_TYPE_MASK)
  {
  case MIFARE_CHIP_TYPE_VIRTUAL:
    return snprintf(dst, size, "(Multiple) Virtual Cards\n");
  case MIFARE_CHIP_TYPE_DESFIRE:
    return snprintf(dst, size, "Mifare DESFire\n");
  case MIFARE_CHIP_TYPE_PLUS:
    return snprintf(dst, size, "Mifare Plus\n");
  default:
    return snprintf(dst, size, "RFU\n");
  }
}

/**
 * Print Mifare memory size
 * Cyclomatic Complexity: 7
 */
static int snprint_mifare_memory_size(char *dst, size_t size, uint8_t mem_size_code)
{
  switch (mem_size_code & MIFARE_CTC_MEMORY_SIZE_MASK)
  {
  case MIFARE_MEM_SIZE_LT_1KB:
    return snprintf(dst, size, "<1 kbyte\n");
  case MIFARE_MEM_SIZE_1KB:
    return snprintf(dst, size, "1 kbyte\n");
  case MIFARE_MEM_SIZE_2KB:
    return snprintf(dst, size, "2 kbyte\n");
  case MIFARE_MEM_SIZE_4KB:
    return snprintf(dst, size, "4 kbyte\n");
  case MIFARE_MEM_SIZE_8KB:
    return snprintf(dst, size, "8 kbyte\n");
  case MIFARE_MEM_SIZE_UNSPECIFIED:
    return snprintf(dst, size, "Unspecified\n");
  default:
    return snprintf(dst, size, "RFU\n");
  }
}

/**
 * Print Mifare chip status
 * Cyclomatic Complexity: 3
 */
static int snprint_mifare_chip_status(char *dst, size_t size, uint8_t chip_status_code)
{
  switch (chip_status_code & MIFARE_CVC_CHIP_STATUS_MASK)
  {
  case MIFARE_CHIP_STATUS_ENGINEERING:
    return snprintf(dst, size, "Engineering sample\n");
  case MIFARE_CHIP_STATUS_RELEASED:
    return snprintf(dst, size, "Released\n");
  default:
    return snprintf(dst, size, "RFU\n");
  }
}

/**
 * Print Mifare chip generation
 * Cyclomatic Complexity: 5
 */
static int snprint_mifare_chip_generation(char *dst, size_t size, uint8_t generation_code)
{
  switch (generation_code & MIFARE_CVC_GENERATION_MASK)
  {
  case MIFARE_GEN_1:
    return snprintf(dst, size, "Generation 1\n");
  case MIFARE_GEN_2:
    return snprintf(dst, size, "Generation 2\n");
  case MIFARE_GEN_3:
    return snprintf(dst, size, "Generation 3\n");
  case MIFARE_GEN_UNSPECIFIED:
    return snprintf(dst, size, "Unspecified\n");
  default:
    return snprintf(dst, size, "RFU\n");
  }
}

/**
 * Print Mifare Virtual Card Selection specifics
 * Cyclomatic Complexity: 6
 */
static int snprint_mifare_vcs_specifics(char *dst, size_t size, uint8_t vcs)
{
  int off = 0;

  off += snprintf(dst + off, size - off, "    * Specifics (Virtual Card Selection):\n");

  if ((vcs & MIFARE_VCS_VCSL_MASK) == 0x00)
  {
    off += snprintf(dst + off, size - off, "      * Only VCSL supported\n");
  }
  else if ((vcs & MIFARE_VCS_VCSL_MASK) == 0x01)
  {
    off += snprintf(dst + off, size - off, "      * VCS, VCSL and SVC supported\n");
  }

  if ((vcs & MIFARE_VCS_SL_MASK) == 0x00)
  {
    off += snprintf(dst + off, size - off, "      * SL1, SL2(?), SL3 supported\n");
  }
  else if ((vcs & MIFARE_VCS_SL_MASK) == 0x02)
  {
    off += snprintf(dst + off, size - off, "      * SL3 only card\n");
  }
  else if ((vcs & MIFARE_VCS_FULL_MASK) == 0x0e)
  {
    off += snprintf(dst + off, size - off, "      * No VCS command supported\n");
  }
  else if ((vcs & MIFARE_VCS_FULL_MASK) == 0x0f)
  {
    off += snprintf(dst + off, size - off, "      * Unspecified\n");
  }
  else
  {
    off += snprintf(dst + off, size - off, "      * RFU\n");
  }

  return off;
}

/**
 * Print Mifare proprietary format (when CIB=0xC1)
 * Cyclomatic Complexity: 9
 */
int snprint_mifare_proprietary(char *dst, size_t size, const nfc_iso14443a_info *pnai, size_t offset)
{
  int off = 0;

  off += snprintf(dst + off, size - off, "    * Tag byte: Mifare or virtual cards of various types\n");

  uint8_t L = pnai->abtAts[offset];
  offset++;

  if (L != (pnai->szAtsLen - offset))
  {
    off += snprintf(dst + off, size - off, "    * Warning: Type Identification Coding length (%i)", L);
    off += snprintf(dst + off, size - off, " not matching Tk length (%" PRIdPTR ")\n",
                    (pnai->szAtsLen - offset));
  }

  // Chip Type Code (CTC)
  if ((pnai->szAtsLen - offset - 2) > 0)
  { // Omit 2 CRC bytes
    uint8_t CTC = pnai->abtAts[offset];
    offset++;

    off += snprintf(dst + off, size - off, "    * Chip Type: ");
    off += snprint_mifare_chip_type(dst + off, size - off, CTC);

    off += snprintf(dst + off, size - off, "    * Memory size: ");
    off += snprint_mifare_memory_size(dst + off, size - off, CTC);
  }

  // Chip Version Code (CVC)
  if ((pnai->szAtsLen - offset) > 0)
  { // Omit 2 CRC bytes
    uint8_t CVC = pnai->abtAts[offset];
    offset++;

    off += snprintf(dst + off, size - off, "    * Chip Status: ");
    off += snprint_mifare_chip_status(dst + off, size - off, CVC);

    off += snprintf(dst + off, size - off, "    * Chip Generation: ");
    off += snprint_mifare_chip_generation(dst + off, size - off, CVC);
  }

  // Virtual Card Selection specifics (VCS)
  if ((pnai->szAtsLen - offset) > 0)
  { // Omit 2 CRC bytes
    uint8_t VCS = pnai->abtAts[offset];
    off += snprint_mifare_vcs_specifics(dst + off, size - off, VCS);
  }

  return off;
}

/**
 * Print COMPACT-TLV format information
 * Cyclomatic Complexity: 5
 */
int snprint_compact_tlv(char *dst, size_t size, uint8_t CIB, const nfc_iso14443a_info *pnai, size_t offset)
{
  int off = 0;

  if (CIB == TK_CIB_COMPACT_TLV)
  {
    off += snprintf(dst + off, size - off,
                    "  * Tk after 0x00 consist of optional consecutive COMPACT-TLV data objects\n");
    off += snprintf(dst + off, size - off,
                    "    followed by a mandatory status indicator (the last three bytes, not in TLV)\n");
    off += snprintf(dst + off, size - off,
                    "    See ISO/IEC 7816-4 8.1.1.3 for more info\n");
  }

  if (CIB == TK_CIB_DIR_DATA_REF)
  {
    off += snprintf(dst + off, size - off, "  * DIR data reference: %02x\n", pnai->abtAts[offset]);
  }

  if (CIB == TK_CIB_COMPACT_TLV_STATUS)
  {
    if (pnai->szAtsLen == offset)
    {
      off += snprintf(dst + off, size - off, "  * No COMPACT-TLV objects found, no status found\n");
    }
    else
    {
      off += snprintf(dst + off, size - off,
                      "  * Tk after 0x80 consist of optional consecutive COMPACT-TLV data objects;\n");
      off += snprintf(dst + off, size - off,
                      "    the last data object may carry a status indicator of one, two or three bytes.\n");
      off += snprintf(dst + off, size - off,
                      "    See ISO/IEC 7816-4 8.1.1.3 for more info\n");
    }
  }

  return off;
}

/**
 * Print historical bytes (Tk) section
 * Cyclomatic Complexity: 6
 */
int snprint_ats_historical_bytes(char *dst, size_t size, const nfc_iso14443a_info *pnai, size_t offset)
{
  int off = 0;

  off += snprintf(dst + off, size - off, "* Historical bytes Tk: ");
  off += snprint_hex(dst + off, size - off, pnai->abtAts + offset, (pnai->szAtsLen - offset));

  uint8_t CIB = pnai->abtAts[offset];
  offset++;

  bool is_proprietary = (CIB != TK_CIB_COMPACT_TLV &&
                         CIB != TK_CIB_DIR_DATA_REF &&
                         (CIB & TK_CIB_COMPACT_TLV_STATUS_MASK) != TK_CIB_COMPACT_TLV_STATUS);

  if (is_proprietary)
  {
    off += snprintf(dst + off, size - off, "  * Proprietary format\n");
    if (CIB == TK_CIB_MIFARE_PROPRIETARY)
    {
      off += snprint_mifare_proprietary(dst + off, size - off, pnai, offset);
    }
  }
  else
  {
    off += snprint_compact_tlv(dst + off, size - off, CIB, pnai, offset);
  }

  return off;
}

/**
 * Lookup table for known ATQA+SAK combinations
 * Returns chip identification string
 */
typedef struct
{
  uint32_t atqa_sak;
  const char *name;
} atqa_sak_match_t;

static const atqa_sak_match_t known_atqa_sak[] = {
    {0x000488, "Mifare Classic 1K Infineon"},
    {0x000298, "Gemplus MPCOS"},
    {0x030428, "JCOP31"},
    {0x004820, "JCOP31 v2.4.1 / v2.2"},
    {0x000428, "JCOP31 v2.3.1"},
    {0x000453, "Fudan FM1208SH01"},
    {0x000820, "Fudan FM1208"},
    {0x000238, "MFC 4K emulated by Nokia 6212 Classic"},
    {0x000838, "MFC 4K emulated by Nokia 6131 NFC"},
};

/**
 * Print fingerprinting information based on ATQA/SAK
 * Cyclomatic Complexity: 11
 */
int snprint_fingerprinting_section(char *dst, size_t size, const nfc_iso14443a_info *pnai)
{
  int off = 0;
  bool found_possible_match = false;

  off += snprintf(dst + off, size - off,
                  "\nFingerprinting based on MIFARE type Identification Procedure:\n");

  uint16_t atqa = (((uint16_t)pnai->abtAtqa[0] & 0xff) << 8) |
                  ((uint16_t)pnai->abtAtqa[1] & 0xff);
  uint8_t sak = (uint8_t)pnai->btSak & 0xff;

  // Match against standard database
  for (size_t i = 0; i < const_ca_size; i++)
  {
    if ((atqa & const_ca[i].mask) == const_ca[i].atqa)
    {
      for (size_t j = 0; j < 8 && const_ca[i].saklist[j] >= 0; j++)
      {
        int sakindex = const_ca[i].saklist[j];
        if ((sak & const_cs[sakindex].mask) == const_cs[sakindex].sak)
        {
          off += snprintf(dst + off, size - off, "* %s%s\n",
                          const_ca[i].type, const_cs[sakindex].type);
          found_possible_match = true;
        }
      }
    }
  }

  // Other matches not in AN10833
  off += snprintf(dst + off, size - off, "Other possible matches based on ATQA & SAK values:\n");

  uint32_t atqasak = (((uint32_t)pnai->abtAtqa[0] & 0xff) << 16) |
                     (((uint32_t)pnai->abtAtqa[1] & 0xff) << 8) |
                     ((uint32_t)pnai->btSak & 0xff);

  for (size_t i = 0; i < sizeof(known_atqa_sak) / sizeof(known_atqa_sak[0]); i++)
  {
    if (atqasak == known_atqa_sak[i].atqa_sak)
    {
      off += snprintf(dst + off, size - off, "* %s\n", known_atqa_sak[i].name);
      found_possible_match = true;
    }
  }

  if (!found_possible_match)
  {
    snprintf(dst + off, size - off, "* Unknown card, sorry\n");
  }

  return off;
}
