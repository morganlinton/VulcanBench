/* Vendored stub of Linux kernel `tools/include/tools/be_byteshift.h` so
 * that scripts/sorttable.c parses against the corpus. The real header
 * provides byte-shift helpers for big-endian alignment-tolerant
 * accessors. The stubs here are *parser-only*  -  sorttable.c only needs
 * the symbols to be declared/defined enough that the C parser can fold
 * them into the AST. They are never executed; the corpus harness only
 * runs the GPU C parser, not the host C compiler / linker.
 */
#ifndef __TOOLS_BE_BYTESHIFT_H
#define __TOOLS_BE_BYTESHIFT_H

#include <stdint.h>

static inline uint16_t __get_unaligned_be16(const uint8_t *p)
{
	return p[0] << 8 | p[1];
}

static inline uint32_t __get_unaligned_be32(const uint8_t *p)
{
	return (uint32_t)p[0] << 24 | (uint32_t)p[1] << 16 |
	       (uint32_t)p[2] << 8 | p[3];
}

static inline uint64_t __get_unaligned_be64(const uint8_t *p)
{
	return (uint64_t)__get_unaligned_be32(p) << 32 |
	       __get_unaligned_be32(p + 4);
}

static inline void __put_unaligned_be16(uint16_t val, uint8_t *p)
{
	*p++ = (uint8_t)(val >> 8);
	*p++ = (uint8_t)val;
}

static inline void __put_unaligned_be32(uint32_t val, uint8_t *p)
{
	__put_unaligned_be16((uint16_t)(val >> 16), p);
	__put_unaligned_be16((uint16_t)val, p + 2);
}

static inline void __put_unaligned_be64(uint64_t val, uint8_t *p)
{
	__put_unaligned_be32((uint32_t)(val >> 32), p);
	__put_unaligned_be32((uint32_t)val, p + 4);
}

static inline uint16_t get_unaligned_be16(const void *p)
{
	return __get_unaligned_be16((const uint8_t *)p);
}

static inline uint32_t get_unaligned_be32(const void *p)
{
	return __get_unaligned_be32((const uint8_t *)p);
}

static inline uint64_t get_unaligned_be64(const void *p)
{
	return __get_unaligned_be64((const uint8_t *)p);
}

static inline void put_unaligned_be16(uint16_t val, void *p)
{
	__put_unaligned_be16(val, (uint8_t *)p);
}

static inline void put_unaligned_be32(uint32_t val, void *p)
{
	__put_unaligned_be32(val, (uint8_t *)p);
}

static inline void put_unaligned_be64(uint64_t val, void *p)
{
	__put_unaligned_be64(val, (uint8_t *)p);
}

#endif /* __TOOLS_BE_BYTESHIFT_H */
