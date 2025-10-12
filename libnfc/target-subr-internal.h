/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 */

/**
 * @file target-subr-internal.h
 * @brief Internal constants and helper functions for target information formatting
 *
 * This header contains magic number definitions and helper function declarations
 * used by target-subr.c for formatting ISO14443 target information.
 * Extracted to reduce complexity and improve maintainability.
 */

#ifndef __NFC_TARGET_SUBR_INTERNAL_H__
#define __NFC_TARGET_SUBR_INTERNAL_H__

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <nfc/nfc.h>

// ============================================================================
// ATQA (Answer To Request Type A) Constants
// ============================================================================

/** ATQA UID size field mask */
#define ATQA_UID_SIZE_MASK 0xC0

/** ATQA UID size field shift */
#define ATQA_UID_SIZE_SHIFT 6

/** ATQA bit frame anticollision field mask */
#define ATQA_BITFRAME_ANTICOL_MASK 0x1F

/** UID size: single (4 bytes) */
#define ATQA_UID_SIZE_SINGLE 0

/** UID size: double (7 bytes) */
#define ATQA_UID_SIZE_DOUBLE 1

/** UID size: triple (10 bytes) */
#define ATQA_UID_SIZE_TRIPLE 2

/** UID size: RFU (Reserved for Future Use) */
#define ATQA_UID_SIZE_RFU 3

// ============================================================================
// SAK (Select Acknowledge) Constants
// ============================================================================

/** SAK bit indicating UID is not complete (cascade bit) */
#define SAK_UID_NOT_COMPLETE 0x04

/** SAK bit indicating ISO/IEC 14443-4 compliance */
#define SAK_ISO14443_4_COMPLIANT 0x20

/** SAK bit indicating ISO/IEC 18092 compliance */
#define SAK_ISO18092_COMPLIANT 0x40

// ============================================================================
// ATS (Answer To Select) Constants
// ============================================================================

/** ATS T0 byte: TA(1) present flag */
#define ATS_T0_TA1_PRESENT 0x10

/** ATS T0 byte: TB(1) present flag */
#define ATS_T0_TB1_PRESENT 0x20

/** ATS T0 byte: TC(1) present flag */
#define ATS_T0_TC1_PRESENT 0x40

/** ATS T0 byte: Maximum frame size mask */
#define ATS_T0_FSCI_MASK 0x0F

/** ATS TA(1): Same bitrate in both directions mandatory */
#define ATS_TA1_SAME_BITRATE 0x80

/** ATS TA(1): PICC to PCD, DS=2 (212 kbps) supported */
#define ATS_TA1_DS2_SUPPORTED 0x10

/** ATS TA(1): PICC to PCD, DS=4 (424 kbps) supported */
#define ATS_TA1_DS4_SUPPORTED 0x20

/** ATS TA(1): PICC to PCD, DS=8 (847 kbps) supported */
#define ATS_TA1_DS8_SUPPORTED 0x40

/** ATS TA(1): PCD to PICC, DR=2 (212 kbps) supported */
#define ATS_TA1_DR2_SUPPORTED 0x01

/** ATS TA(1): PCD to PICC, DR=4 (424 kbps) supported */
#define ATS_TA1_DR4_SUPPORTED 0x02

/** ATS TA(1): PCD to PICC, DR=8 (847 kbps) supported */
#define ATS_TA1_DR8_SUPPORTED 0x04

/** ATS TA(1): Unknown/error bit */
#define ATS_TA1_ERROR_BIT 0x08

/** ATS TB(1): Frame Waiting Time mask */
#define ATS_TB1_FWI_MASK 0xF0

/** ATS TB(1): Frame Waiting Time shift */
#define ATS_TB1_FWI_SHIFT 4

/** ATS TB(1): Start-up Frame Guard Time mask */
#define ATS_TB1_SFGI_MASK 0x0F

/** ATS TC(1): Node Address supported */
#define ATS_TC1_NAD_SUPPORTED 0x01

/** ATS TC(1): Card IDentifier (CID) supported */
#define ATS_TC1_CID_SUPPORTED 0x02

// ============================================================================
// Historical Bytes (Tk) Constants
// ============================================================================

/** Historical bytes CIB: Optional COMPACT-TLV format */
#define TK_CIB_COMPACT_TLV 0x00

/** Historical bytes CIB: DIR data reference */
#define TK_CIB_DIR_DATA_REF 0x10

/** Historical bytes CIB: COMPACT-TLV with status (mask) */
#define TK_CIB_COMPACT_TLV_STATUS_MASK 0xF0

/** Historical bytes CIB: COMPACT-TLV with status (value) */
#define TK_CIB_COMPACT_TLV_STATUS 0x80

/** Historical bytes CIB: Mifare proprietary format tag */
#define TK_CIB_MIFARE_PROPRIETARY 0xC1

/** Mifare Chip Type Code mask */
#define MIFARE_CTC_CHIP_TYPE_MASK 0xF0

/** Mifare Chip Type Code memory size mask */
#define MIFARE_CTC_MEMORY_SIZE_MASK 0x0F

/** Mifare Chip Type: Multiple/Virtual Cards */
#define MIFARE_CHIP_TYPE_VIRTUAL 0x00

/** Mifare Chip Type: DESFire */
#define MIFARE_CHIP_TYPE_DESFIRE 0x10

/** Mifare Chip Type: Plus */
#define MIFARE_CHIP_TYPE_PLUS 0x20

/** Mifare Memory Size: <1 kbyte */
#define MIFARE_MEM_SIZE_LT_1KB 0x00

/** Mifare Memory Size: 1 kbyte */
#define MIFARE_MEM_SIZE_1KB 0x01

/** Mifare Memory Size: 2 kbyte */
#define MIFARE_MEM_SIZE_2KB 0x02

/** Mifare Memory Size: 4 kbyte */
#define MIFARE_MEM_SIZE_4KB 0x03

/** Mifare Memory Size: 8 kbyte */
#define MIFARE_MEM_SIZE_8KB 0x04

/** Mifare Memory Size: Unspecified */
#define MIFARE_MEM_SIZE_UNSPECIFIED 0x0F

/** Mifare Chip Version Code chip status mask */
#define MIFARE_CVC_CHIP_STATUS_MASK 0xF0

/** Mifare Chip Version Code generation mask */
#define MIFARE_CVC_GENERATION_MASK 0x0F

/** Mifare Chip Status: Engineering sample */
#define MIFARE_CHIP_STATUS_ENGINEERING 0x00

/** Mifare Chip Status: Released */
#define MIFARE_CHIP_STATUS_RELEASED 0x20

/** Mifare Chip Generation: Generation 1 */
#define MIFARE_GEN_1 0x00

/** Mifare Chip Generation: Generation 2 */
#define MIFARE_GEN_2 0x01

/** Mifare Chip Generation: Generation 3 */
#define MIFARE_GEN_3 0x02

/** Mifare Chip Generation: Unspecified */
#define MIFARE_GEN_UNSPECIFIED 0x0F

/** Mifare Virtual Card Selection specifics mask */
#define MIFARE_VCS_VCSL_MASK 0x09

/** Mifare Virtual Card Selection security level mask */
#define MIFARE_VCS_SL_MASK 0x0E

/** Mifare Virtual Card Selection full mask */
#define MIFARE_VCS_FULL_MASK 0x0F

// ============================================================================
// UID Constants
// ============================================================================

/** UID first byte: Random UID indicator */
#define UID_RANDOM_ID 0x08

// ============================================================================
// Timing Constants (in carrier frequency cycles)
// ============================================================================

/** Carrier frequency in Hz */
#define FC_HZ 13560000.0

/** Conversion factor: 256 * 16 */
#define TIMING_FACTOR 4096.0

// ============================================================================
// Helper Function Declarations
// ============================================================================

/**
 * @brief Print ATQA (Answer To Request Type A) section
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @param verbose Enable verbose output
 * @return Number of characters written
 */
int snprint_atqa_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose);

/**
 * @brief Print UID (Unique Identifier) section
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @param verbose Enable verbose output
 * @return Number of characters written
 */
int snprint_uid_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose);

/**
 * @brief Print SAK (Select Acknowledge) section
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @param verbose Enable verbose output
 * @return Number of characters written
 */
int snprint_sak_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose);

/**
 * @brief Print ATS (Answer To Select) section
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @param verbose Enable verbose output
 * @return Number of characters written
 */
int snprint_ats_section(char *dst, size_t size, const nfc_iso14443a_info *pnai, bool verbose);

/**
 * @brief Print bitrate capability information from TA(1)
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param TA TA(1) byte value
 * @return Number of characters written
 */
int snprint_ats_bitrate_capability(char *dst, size_t size, uint8_t TA);

/**
 * @brief Print frame timing information from TB(1)
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param TB TB(1) byte value
 * @return Number of characters written
 */
int snprint_ats_frame_timing(char *dst, size_t size, uint8_t TB);

/**
 * @brief Print node address and CID support from TC(1)
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param TC TC(1) byte value
 * @return Number of characters written
 */
int snprint_ats_node_cid_support(char *dst, size_t size, uint8_t TC);

/**
 * @brief Print historical bytes (Tk) section
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @param offset Offset to historical bytes in ATS
 * @return Number of characters written
 */
int snprint_ats_historical_bytes(char *dst, size_t size, const nfc_iso14443a_info *pnai, size_t offset);

/**
 * @brief Print Mifare proprietary format (when CIB=0xC1)
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @param offset Offset to start of Mifare data
 * @return Number of characters written
 */
int snprint_mifare_proprietary(char *dst, size_t size, const nfc_iso14443a_info *pnai, size_t offset);

/**
 * @brief Print COMPACT-TLV format information
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param CIB Category Indicator Byte
 * @param pnai ISO14443A info structure
 * @param offset Offset to COMPACT-TLV data
 * @return Number of characters written
 */
int snprint_compact_tlv(char *dst, size_t size, uint8_t CIB, const nfc_iso14443a_info *pnai, size_t offset);

/**
 * @brief Print fingerprinting information based on ATQA/SAK
 *
 * @param dst Output buffer
 * @param size Size of output buffer
 * @param pnai ISO14443A info structure
 * @return Number of characters written
 */
int snprint_fingerprinting_section(char *dst, size_t size, const nfc_iso14443a_info *pnai);

#endif /* __NFC_TARGET_SUBR_INTERNAL_H__ */
