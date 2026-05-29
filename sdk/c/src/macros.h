#include <stdint.h>

#define REQUIRE_WITH_MSG(cond, msg)                                       \
  do {                                                                    \
    if (!(cond)) {                                                        \
      fprintf(stderr, "\n\nREQUIRE FAILED: %s:%d\n", __FILE__, __LINE__); \
      if (msg != NULL) {                                                  \
        fprintf(stderr, ": %s", (char*)msg);                              \
        fflush(stderr);                                                   \
      }                                                                   \
      fprintf(stderr, "\n");                                              \
      fflush(stderr);                                                     \
      abort();                                                            \
    }                                                                     \
  } while (0)

#define REQUIRE(cond) REQUIRE_WITH_MSG(cond, NULL)

#define TODO(msg)                                       \
  do {                                                  \
    fprintf(stderr, "TODO: %s:%d", __FILE__, __LINE__); \
    if (msg != NULL) {                                  \
      fprintf(stderr, ": %s", (char*)msg);              \
    }                                                   \
    fprintf(stderr, "\n");                              \
    fflush(stderr);                                     \
    abort();                                            \
  } while (0)

static inline uint32_t popcount32(uint32_t x)
{
#if defined(_MSC_VER)
  return (uint32_t)__popcnt(x);
#elif defined(__GNUC__) || defined(__clang__)
  return (uint32_t)__builtin_popcount(x);
#else
  // Portable fallback: Hacker's Delight
  x = x - ((x >> 1) & 0x55555555u);
  x = (x & 0x33333333u) + ((x >> 2) & 0x33333333u);
  x = (x + (x >> 4)) & 0x0F0F0F0Fu;
  x = x * 0x01010101u;
  return (uint32_t)(x >> 24);
#endif
}

static inline uint32_t popcount64(uint64_t x)
{
#if defined(_MSC_VER)
  return (uint32_t)__popcnt64(x);
#elif defined(__GNUC__) || defined(__clang__)
  return (uint32_t)__builtin_popcountll(x);
#else
  // Portable fallback: Hacker's Delight
  x = x - ((x >> 1) & 0x5555555555555555ull);
  x = (x & 0x3333333333333333ull) + ((x >> 2) & 0x3333333333333333ull);
  x = (x + (x >> 4)) & 0x0F0F0F0F0F0F0F0Full;
  x = x * 0x0101010101010101ull;
  return (uint32_t)(x >> 56);
#endif
}

// The purpose of this struct is to check for maximum alignment compatibility of other
// types.
typedef union
{
  long long   ll;
  long double ld;
  void*       p;
} _MaxAlignCompat;
