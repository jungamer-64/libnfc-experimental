/* ISO14443 CRC functions */
#include <stdint.h>
#include <stddef.h>
#include <nfc/nfc.h>

void iso14443a_crc_append(uint8_t *pbtData, size_t szLen)
{
  uint16_t crc = 0x6363;
  for (size_t i = 0; i < szLen; i++) {
    uint8_t bt = pbtData[i];
    bt = (bt ^ (uint8_t)(crc & 0x00FF));
    bt = (bt ^ (bt << 4));
    crc = (crc >> 8) ^ ((uint16_t)bt << 8) ^ ((uint16_t)bt << 3) ^ ((uint16_t)bt >> 4);
  }
  pbtData[szLen] = (uint8_t)(crc & 0xFF);
  pbtData[szLen + 1] = (uint8_t)((crc >> 8) & 0xFF);
}

void iso14443b_crc_append(uint8_t *pbtData, size_t szLen)
{
  uint16_t crc = 0xFFFF;
  for (size_t i = 0; i < szLen; i++) {
    uint8_t bt = pbtData[i];
    bt = (bt ^ (uint8_t)(crc & 0x00FF));
    bt = (bt ^ (bt << 4));
    crc = (crc >> 8) ^ ((uint16_t)bt << 8) ^ ((uint16_t)bt << 3) ^ ((uint16_t)bt >> 4);
  }
  crc = ~crc;
  pbtData[szLen] = (uint8_t)(crc & 0xFF);
  pbtData[szLen + 1] = (uint8_t)((crc >> 8) & 0xFF);
}

void iso14443a_crc(uint8_t *pbtData, size_t szLen, uint8_t *pbtCrc)
{
  uint16_t crc = 0x6363;
  for (size_t i = 0; i < szLen; i++) {
    uint8_t bt = pbtData[i];
    bt = (bt ^ (uint8_t)(crc & 0x00FF));
    bt = (bt ^ (bt << 4));
    crc = (crc >> 8) ^ ((uint16_t)bt << 8) ^ ((uint16_t)bt << 3) ^ ((uint16_t)bt >> 4);
  }
  pbtCrc[0] = (uint8_t)(crc & 0xFF);
  pbtCrc[1] = (uint8_t)((crc >> 8) & 0xFF);
}

uint8_t *iso14443a_locate_historical_bytes(uint8_t *pbtAts, size_t szAts, size_t *pszTk)
{
  if (szAts < 1) {
    *pszTk = 0;
    return NULL;
  }
  uint8_t t0 = pbtAts[0];
  size_t offset = 1;
  if (t0 & 0x10)
    offset++;
  if (t0 & 0x20)
    offset++;
  if (t0 & 0x40)
    offset++;
  if (offset >= szAts) {
    *pszTk = 0;
    return NULL;
  }
  *pszTk = szAts - offset;
  return pbtAts + offset;
}

void iso14443_cascade_uid(const uint8_t *abtUID, const size_t szUID, uint8_t *pbtCascadedUID, size_t *pszCascadedUID)
{
  switch (szUID) {
    case 4:
      *pszCascadedUID = szUID;
      for (size_t i = 0; i < szUID; i++)
        pbtCascadedUID[i] = abtUID[i];
      break;
    case 7:
      *pszCascadedUID = 8;
      pbtCascadedUID[0] = 0x88;
      for (size_t i = 0; i < 3; i++)
        pbtCascadedUID[i + 1] = abtUID[i];
      for (size_t i = 3; i < szUID; i++)
        pbtCascadedUID[i + 1] = abtUID[i];
      break;
    case 10:
      *pszCascadedUID = 12;
      pbtCascadedUID[0] = 0x88;
      for (size_t i = 0; i < 3; i++)
        pbtCascadedUID[i + 1] = abtUID[i];
      pbtCascadedUID[4] = 0x88;
      for (size_t i = 3; i < 6; i++)
        pbtCascadedUID[i + 2] = abtUID[i];
      for (size_t i = 6; i < szUID; i++)
        pbtCascadedUID[i + 2] = abtUID[i];
      break;
    default:
      *pszCascadedUID = 0;
      break;
  }
}
