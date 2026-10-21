/* Vendored stub of Linux kernel `tools/include/tools/le_byteshift.h`  - 
 * little-endian counterpart to be_byteshift.h. Parser-only stubs;
 * sufficient for the corpus to fold scripts/sorttable.c into the AST.
 */
#ifndef __TOOLS_LE_BYTESHIFT_H
#define __TOOLS_LE_BYTESHIFT_H

#include <stdint.h>

static inline uint16_t __get_unaligned_le16(const uint8_t *p)
{
	return p[0] | p[1] << 8;
}

static inline uint32_t __get_unaligned_le32(const uint8_t *p)
{
	return p[0] | (uint32_t)p[1] << 8 |
	       (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24;
}

static inline uint64_t __get_unaligned_le64(const uint8_t *p)
{
	return (uint64_t)__get_unaligned_le32(p + 4) << 32 |
	       __get_unaligned_le32(p);
}

static inline void __put_unaligned_le16(uint16_t val, uint8_t *p)
{
	*p++ = (uint8_t)val;
	*p++ = (uint8_t)(val >> 8);
}

static inline void __put_unaligned_le32(uint32_t val, uint8_t *p)
{
	__put_unaligned_le16((uint16_t)val, p);
	__put_unaligned_le16((uint16_t)(val >> 16), p + 2);
}

static inline void __put_unaligned_le64(uint64_t val, uint8_t *p)
{
	__put_unaligned_le32((uint32_t)val, p);
	__put_unaligned_le32((uint32_t)(val >> 32), p + 4);
}

static inline uint16_t get_unaligned_le16(const void *p)
{
	return __get_unaligned_le16((const uint8_t *)p);
}

static inline uint32_t get_unaligned_le32(const void *p)
{
	return __get_unaligned_le32((const uint8_t *)p);
}

static inline uint64_t get_unaligned_le64(const void *p)
{
	return __get_unaligned_le64((const uint8_t *)p);
}

static inline void put_unaligned_le16(uint16_t val, void *p)
{
	__put_unaligned_le16(val, (uint8_t *)p);
}

static inline void put_unaligned_le32(uint32_t val, void *p)
{
	__put_unaligned_le32(val, (uint8_t *)p);
}

static inline void put_unaligned_le64(uint64_t val, void *p)
{
	__put_unaligned_le64(val, (uint8_t *)p);
}

#endif /* __TOOLS_LE_BYTESHIFT_H */
