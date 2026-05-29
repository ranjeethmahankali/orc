#include "container.h"

#include <assert.h>
#include <ctype.h>
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "macros.h"
#include "status.h"

void* _arr_grow(void* ptr, size_t elemsize)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h == NULL) {
    h = malloc(sizeof *h + elemsize);
    if (h == NULL)
      return NULL;
    h->count    = 0;
    h->capacity = 1;
    ptr         = h + 1;
  }
  else if (h->count == h->capacity) {
    size_t const newcap = h->capacity * 2;
    h                   = realloc(h, sizeof *h + newcap * elemsize);
    if (h == NULL)
      return NULL;
    h->capacity = newcap;
    ptr         = h + 1;
  }
  REQUIRE_WITH_MSG(h->count <= h->capacity, "Count cannot exceed capacity");
  return ptr;
}

void* _arr_grow_capacity(void* ptr, size_t const elemsize, size_t const nelems)
{
  // Handle the special case where ptr is NULL and nelems is 0
  // No allocation needed, just return the NULL ptr
  if (ptr == NULL && nelems == 0) {
    return ptr;  // Return NULL, which is valid for an empty array
  }
  _ArrHeader* h = _arr_header(ptr);
  if (h == NULL) {
    h = malloc(sizeof *h + elemsize * nelems);
    if (h == NULL)
      return NULL;
    h->count    = 0;
    h->capacity = nelems;
    ptr         = h + 1;
  }
  else if (h->capacity < nelems) {
    h = realloc(h, sizeof *h + nelems * elemsize);
    if (h == NULL)
      return NULL;
    h->capacity = nelems;
    ptr         = h + 1;
  }
  return ptr;
}

Status _arr_remove_impl(void* ptr, size_t const idx, size_t const elemsize)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h && (idx < h->count)) {
    void*        dst = (char*)ptr + idx * elemsize;
    void*        src = (char*)dst + elemsize;
    size_t const len = --(h->count) - idx;
    if (len)
      memmove(dst, src, len * elemsize);
    return OK;
  }
  return OUT_OF_BOUNDS;
}

void* _arr_resize_impl(void* ptr, size_t const elemsize, size_t const count)
{
  size_t const before = arr_len(ptr);
  if (before < count) {  // Needs to grow.
    ptr = _arr_grow_capacity(ptr, elemsize, count);
    if (ptr == NULL) {
      return NULL;
    }
    char* dst = (char*)ptr + (before * elemsize);
    memset(dst, 0, (count - before) * elemsize);
  }
  _ArrHeader* h = _arr_header(ptr);
  if (h)
    h->count = count;
  return ptr;
}

void arr_clear(void* ptr)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h)
    h->count = 0;
}

void _arr_fill_impl(void*             arr,
                    void const* const elem,
                    size_t const      count,
                    size_t const      elemsize)
{
  if (count == 0)
    return;
  char* dst = (char*)arr;
  char* src = (char*)elem;
  memcpy(dst, src, elemsize);
  src = dst;
  dst += elemsize;
  size_t n      = count >> 1;
  size_t filled = 1;
  while (n) {
    memcpy(dst, src, filled * elemsize);
    dst += filled * elemsize;
    filled <<= 1;
    n >>= 1;
  }
  if (filled < count) {
    n = count - filled;
    memcpy(dst, dst - n * elemsize, n * elemsize);
    dst += n * elemsize;
  }
  REQUIRE_WITH_MSG(dst == (char*)arr + count * elemsize,
                   "Should have written up to the end of the array.");
}

Status _arr_remove_range_impl(void*        ptr,
                              size_t const start,
                              size_t const stop,
                              size_t const elemsize)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h && (start <= stop) && (start <= h->count) && (stop <= h->count)) {
    if (start == stop || start == h->count) {
      return OK;
    }
    char*        dst      = (char*)ptr + start * elemsize;
    size_t const nremoved = stop - start;
    size_t const nshift   = h->count - stop;
    if (nshift)
      memmove(dst, dst + nremoved * elemsize, nshift * elemsize);
    h->count -= nremoved;
    return OK;
  }
  return OUT_OF_BOUNDS;
}

bool arr_is_empty(void* ptr)
{
  _ArrHeader* h = _arr_header(ptr);
  return h == NULL || h->count == 0;
}

// ========== Queue ==========

void* _que_push_grow(void* ptr, size_t const elemsize)
{
  _QueueHeader* h = _que_header(ptr);
  if (h == NULL) {
    // Having a min size that is larger than 1, avoids a bunch of edge cases in the ring
    // buffer logic.
    size_t const MIN_QUEUE_SIZE = 2;
    h                           = malloc(sizeof *h + elemsize * MIN_QUEUE_SIZE);
    if (h == NULL)
      return NULL;
    h->back     = 0;
    h->capacity = MIN_QUEUE_SIZE;
    h->front    = 0;
    ptr         = h + 1;
  }
  else if ((1 + h->back) % h->capacity == h->front) {  // Need to grow.
    size_t const newcap = h->capacity * 2;
    h                   = realloc(h, sizeof *h + newcap * elemsize);
    if (h == NULL)
      return NULL;
    ptr = h + 1;
    if (h->back < h->front) {
      memmove((char*)ptr + h->capacity * elemsize, ptr, h->back * elemsize);
      h->back += h->capacity;
    }
    h->capacity = newcap;
  }
  return ptr;
}

size_t _que_pushed_back(void* ptr)
{
  _QueueHeader* h = _que_header(ptr);
  REQUIRE_WITH_MSG(h != NULL && ptr != NULL,
                   "This function shouldn't be called on empty queues");
  size_t const newback = (1 + h->back) % h->capacity;
  REQUIRE_WITH_MSG(
    newback != h->front,
    "This should never be called on a queue that doesn't have a large enough buffer");
  size_t const back = h->back;
  h->back           = newback;
  return back;
}

size_t _que_popped_front(void* ptr)
{
  _QueueHeader* h = _que_header(ptr);
  REQUIRE_WITH_MSG(h != NULL && ptr != NULL && h->front != h->back,
                   "This function shouldn't be called on empty queues");
  size_t const front = h->front;
  h->front           = (1 + h->front) % h->capacity;
  return front;
}

bool que_is_empty(void* ptr)
{
  _QueueHeader* h = _que_header(ptr);
  return h == NULL || h->front == h->back;
}

size_t que_len(void* ptr)
{
  _QueueHeader* h = _que_header(ptr);
  if (h) {
    if (h->front < h->back)
      return h->back - h->front;
    else if (h->front > h->back)
      return (h->capacity - h->front) + h->back;
  }
  return 0;
}

// ========== Hashmap ==========

typedef struct
{
  size_t bucket;
  size_t slot;
  enum
  {
    INVALID,
    EMPTY,
    OCCUPIED,
  } type;
} Slot;

static Slot find_empty_slot(size_t const       hash,
                            _HashBucket const* buckets,
                            size_t const       nb)
{
  size_t const ibucket = hash % nb;
  for (size_t probe = 0; probe < nb; ++probe) {
    size_t const itry = (ibucket + probe) % nb;
    for (size_t slot = 0; slot < HMAP_BUCKET_SIZE; ++slot) {
      if (buckets[itry].hash[slot] == 0) {
        return (Slot) {.bucket = itry, .slot = slot, .type = EMPTY};
      }
    }
  }
  return (Slot) {.type = INVALID};
}

static Slot find_writable_slot_bin(size_t const       hash,
                                   _HashBucket const* buckets,
                                   size_t const       nb,
                                   void*              keyptr,
                                   size_t const       keysize,
                                   void*              ptr,
                                   size_t const       kvsize)
{
  size_t const ibucket = hash % nb;
  for (size_t probe = 0; probe < nb; ++probe) {
    size_t const itry = (ibucket + probe) % nb;
    for (size_t slot = 0; slot < HMAP_BUCKET_SIZE; ++slot) {
      if (buckets[itry].hash[slot] == 0) {
        return (Slot) {.bucket = itry, .slot = slot, .type = EMPTY};
      }
      else if (buckets[itry].hash[slot] == hash &&
               0 == memcmp((char*)ptr + kvsize * (size_t)buckets[itry].index[slot],
                           keyptr,
                           keysize)) {
        return (Slot) {.bucket = itry, .slot = slot, .type = OCCUPIED};
      }
    }
  }
  return (Slot) {.type = INVALID};
}

static Slot find_writable_slot_str(size_t const       hash,
                                   _HashBucket const* buckets,
                                   size_t const       nb,
                                   char const*        key,
                                   void*              pairs,
                                   size_t const       kvsize)
{
  size_t const ibucket = hash % nb;
  for (size_t probe = 0; probe < nb; ++probe) {
    size_t const itry = (ibucket + probe) % nb;
    for (size_t slot = 0; slot < HMAP_BUCKET_SIZE; ++slot) {
      ptrdiff_t const index = buckets[itry].index[slot];
      char**          keyptr =
        index >= 0 ? (char**)(void*)((char*)pairs + kvsize * (size_t)index) : NULL;
      if (buckets[itry].hash[slot] == 0) {
        return (Slot) {.bucket = itry, .slot = slot, .type = EMPTY};
      }
      else if (buckets[itry].hash[slot] == hash && 0 == strcmp(*keyptr, key)) {
        return (Slot) {.bucket = itry, .slot = slot, .type = OCCUPIED};
      }
    }
  }
  return (Slot) {.type = INVALID};
}

static Slot find_readable_slot_bin(size_t const       hash,
                                   _HashBucket* const buckets,
                                   size_t const       nb,
                                   void*              keyptr,
                                   size_t const       keysize,
                                   void*              pairs,
                                   size_t const       kvsize)
{
  size_t const ibucket = hash % nb;
  for (size_t probe = 0; probe < nb; ++probe) {
    size_t const itry = (ibucket + probe) % nb;
    for (size_t slot = 0; slot < HMAP_BUCKET_SIZE; ++slot) {
      if (buckets[itry].hash[slot] == 0) {
        // No need to keep probing if we reach the end of this bucket.
        return (Slot) {.type = INVALID};
      }
      else if (buckets[itry].hash[slot] == hash &&
               0 == memcmp((char*)pairs + kvsize * (size_t)buckets[itry].index[slot],
                           keyptr,
                           keysize)) {
        return (Slot) {.bucket = itry, .slot = slot, .type = OCCUPIED};
      }
    }
  }
  return (Slot) {.type = INVALID};
}

static Slot find_readable_slot_str(size_t const       hash,
                                   _HashBucket* const buckets,
                                   size_t const       nb,
                                   char const*        key,
                                   void*              pairs,
                                   size_t const       kvsize)
{
  size_t const ibucket = hash % nb;
  for (size_t probe = 0; probe < nb; ++probe) {
    size_t const itry = (ibucket + probe) % nb;
    for (size_t slot = 0; slot < HMAP_BUCKET_SIZE; ++slot) {
      ptrdiff_t const index = buckets[itry].index[slot];
      char**          keyptr =
        index >= 0 ? (char**)(void*)((char*)pairs + kvsize * (size_t)index) : NULL;
      if (buckets[itry].hash[slot] == 0) {
        // No need to keep probing if we reach the end of this bucket.
        return (Slot) {.type = INVALID};
      }
      else if (buckets[itry].hash[slot] == hash && 0 == strcmp(*keyptr, key)) {
        return (Slot) {.bucket = itry, .slot = slot, .type = OCCUPIED};
      }
    }
  }
  return (Slot) {.type = INVALID};
}

static void _hmap_redist_buckets(_HashBucket* src,
                                 size_t const nsrc,
                                 _HashBucket* dst,
                                 size_t const ndst,
                                 size_t*      slots)
{
  memset(dst, 0, ndst * sizeof *dst);
  if (nsrc == 0)
    return;
  _HashBucket const* src_end = src + nsrc;
  while (src != src_end) {
    for (size_t i = 0; i < HMAP_BUCKET_SIZE; ++i) {
      size_t const    hash  = src->hash[i];
      ptrdiff_t const index = src->index[i];
      if (hash == 0 || hash == 1)
        continue;
      Slot slot = find_empty_slot(hash, dst, ndst);
      switch (slot.type) {
      case EMPTY:
        dst[slot.bucket].hash[slot.slot]  = hash;
        dst[slot.bucket].index[slot.slot] = index;
        slots[index]                      = slot.bucket * HMAP_BUCKET_SIZE + slot.slot;
        break;
      default:
        REQUIRE_WITH_MSG(false, "Fatal error in hash map. Sorry!");
      }
    }
    ++src;
  }
}

void* _hmap_grow_size(void* ptr, size_t const kvsize, size_t nelems)
{
  if (nelems % HMAP_BUCKET_SIZE) {  // Make the element count a multiple of bucket size.
    nelems += HMAP_BUCKET_SIZE - (nelems % HMAP_BUCKET_SIZE);
  }
  size_t const      nbuckets = nelems / HMAP_BUCKET_SIZE;
  _HashTableHeader* h        = _hmap_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + kvsize * nelems;
    h                    = malloc(bufsize);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->n_total   = nelems;
    h->n_removed = 0;
    h->n_used    = 0;
    h->buckets   = NULL;
    arr_resize(h->buckets, nbuckets);
    size_t const INVALID_IDX = SIZE_MAX;
    arr_resize(h->slots, nelems);
    arr_fill(h->slots, INVALID_IDX);
    ptr = h + 1;
  }
  else if (h->n_total < nelems) {
    h = realloc(h, sizeof *h + kvsize * nelems);
    if (h == NULL)
      return NULL;
    size_t const oldtotal = h->n_total;
    REQUIRE_WITH_MSG((oldtotal % HMAP_BUCKET_SIZE) == 0, "Hash map is corrupt. Sorry!");
    arr_clear(h->temp_buckets);
    arr_resize(h->temp_buckets, nbuckets);  // Zeroes out.
    arr_resize(h->slots, nelems);
    size_t const INVALID_IDX = SIZE_MAX;
    arr_fill(h->slots, INVALID_IDX);
    ptr = h + 1;
    _hmap_redist_buckets(
      h->buckets, oldtotal / HMAP_BUCKET_SIZE, h->temp_buckets, nbuckets, h->slots);
    h->n_total   = nelems;
    h->n_removed = 0;  // We don't carry over tombstones during redistribution.
    {                  // Swap the buckets.
      _HashBucket* temp = h->temp_buckets;
      h->temp_buckets   = h->buckets;
      h->buckets        = temp;
    }
  }
  return ptr;
}

// Simple FNV-1a hash function for strings
static inline size_t fnv_hash_str(const char* key)
{
  if (!key)
    return 0;
  const size_t FNV_OFFSET_BASIS = 14695981039346656037ULL;
  const size_t FNV_PRIME        = 1099511628211ULL;
  size_t       hash             = FNV_OFFSET_BASIS;
  while (*key) {
    hash ^= (size_t)*key++;
    hash *= FNV_PRIME;
  }
  return hash;
}

// FNV-1a hash for arbitrary data
static inline size_t fnv_hash_bytes(const void* data, size_t len)
{
  if (!data || len == 0)
    return 0;
  const size_t         FNV_OFFSET_BASIS = 14695981039346656037ULL;
  const size_t         FNV_PRIME        = 1099511628211ULL;
  size_t               hash             = FNV_OFFSET_BASIS;
  const unsigned char* bytes            = (const unsigned char*)data;
  for (size_t i = 0; i < len; i++) {
    hash ^= bytes[i];
    hash *= FNV_PRIME;
  }
  // Avoid sentinel values.
  if (hash < 2)
    hash += 2;
  return hash;
}

static inline void* _hmap_grow_if_needed(void* ptr, size_t const kvsize)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h == NULL) {
    size_t const newtotal = HMAP_BUCKET_SIZE;
    ptr                   = _hmap_grow_size(ptr, kvsize, newtotal);
    h                     = _hmap_header(ptr);
  }
  else if ((h->n_used + h->n_removed) >= (3 * h->n_total / 4)) {
    size_t newtotal = h->n_total * 2;
    ptr             = _hmap_grow_size(ptr, kvsize, newtotal);
    h               = _hmap_header(ptr);
  }
  REQUIRE_WITH_MSG(h->n_total % HMAP_BUCKET_SIZE == 0, "Corrupted hash map. Sorry.");
  return ptr;
}

size_t _hmap_insert_bin_impl(void**       ptr,
                             size_t const kvsize,
                             void*        keyptr,
                             size_t const keysize)
{
  *ptr                   = _hmap_grow_if_needed(*ptr, kvsize);
  _HashTableHeader* h    = _hmap_header(*ptr);
  size_t const      hash = fnv_hash_bytes(keyptr, keysize);
  Slot const        slot = find_writable_slot_bin(
    hash, h->buckets, h->n_total / HMAP_BUCKET_SIZE, keyptr, keysize, *ptr, kvsize);
  switch (slot.type) {
  case INVALID:  // Should never happen.
    REQUIRE_WITH_MSG(false, "[ERROR] Hash map slots are all full.");
    return SIZE_MAX;
  case EMPTY: {
    size_t const index = h->n_used++;
    REQUIRE_WITH_MSG(h->n_used < h->n_total, "Hashmap is not the correct size");
    memcpy((char*)(*ptr) + index * kvsize, keyptr, keysize);
    h->buckets[slot.bucket].hash[slot.slot]  = hash;
    h->buckets[slot.bucket].index[slot.slot] = (ptrdiff_t)index;
    h->slots[index]                          = slot.bucket * HMAP_BUCKET_SIZE + slot.slot;
    return index;
  }
  case OCCUPIED:
    return (size_t)h->buckets[slot.bucket].index[slot.slot];
  };
  return SIZE_MAX;  // This should never happen.
}

size_t _hmap_insert_str_impl(void** ptr, size_t const kvsize, char const* key)
{
  *ptr                   = _hmap_grow_if_needed(*ptr, kvsize);
  _HashTableHeader* h    = _hmap_header(*ptr);
  size_t const      hash = fnv_hash_str(key);
  Slot const        slot = find_writable_slot_str(
    hash, h->buckets, h->n_total / HMAP_BUCKET_SIZE, key, *ptr, kvsize);
  switch (slot.type) {
  case INVALID:  // Should never happen.
    REQUIRE_WITH_MSG(false, "[ERROR] Hash map slots are all full.");
    return SIZE_MAX;
  case EMPTY: {
    size_t const index = h->n_used++;
    REQUIRE_WITH_MSG(h->n_used < h->n_total, "Hashmap is not the correct size");
    *(char const**)(void*)((char*)(*ptr) + index * kvsize) = key;
    h->buckets[slot.bucket].hash[slot.slot]                = hash;
    h->buckets[slot.bucket].index[slot.slot]               = (ptrdiff_t)index;
    h->slots[index] = slot.bucket * HMAP_BUCKET_SIZE + slot.slot;
    return index;
  }
  case OCCUPIED:
    return (size_t)h->buckets[slot.bucket].index[slot.slot];
  };
  REQUIRE_WITH_MSG(false, "Unreachable");
  return SIZE_MAX;  // This should never happen.
}

size_t hmap_len(void* ptr)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h)
    return h->n_used;
  return 0;
}

void _hmap_free_impl(void* ptr)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h) {
    arr_free(h->buckets);
    arr_free(h->temp_buckets);
    arr_free(h->slots);
    free(h);
  }
}

void* _hmap_get_bin_impl(void*        ptr,
                         size_t const kvsize,
                         void*        keyptr,
                         size_t const keysize)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h) {
    size_t const hash = fnv_hash_bytes(keyptr, keysize);
    Slot const   slot = find_readable_slot_bin(
      hash, h->buckets, h->n_total / HMAP_BUCKET_SIZE, keyptr, keysize, ptr, kvsize);
    switch (slot.type) {
    case EMPTY:  // Should never happen.
      REQUIRE_WITH_MSG(false, "[ERROR] Hash map slots are all full.");
      return NULL;
    case INVALID:
      return NULL;
    case OCCUPIED:
      return (char*)ptr + kvsize * (size_t)h->buckets[slot.bucket].index[slot.slot];
    };
  }
  return NULL;
}

void* _hmap_get_str_impl(void* ptr, size_t const kvsize, char const* key)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h) {
    size_t const hash = fnv_hash_str(key);
    Slot const   slot = find_readable_slot_str(
      hash, h->buckets, h->n_total / HMAP_BUCKET_SIZE, key, ptr, kvsize);
    switch (slot.type) {
    case EMPTY:  // Should never happen.
      REQUIRE_WITH_MSG(false, "[ERROR] Hash map slots are all full.");
      return NULL;
    case INVALID:
      return NULL;
    case OCCUPIED:
      return (char*)ptr + kvsize * (size_t)h->buckets[slot.bucket].index[slot.slot];
    };
  }
  return NULL;
}

static void _hmap_compact(void* ptr)
{
  _HashTableHeader* h        = _hmap_header(ptr);
  size_t const      nbuckets = (h->n_total / HMAP_BUCKET_SIZE);
  arr_clear(h->temp_buckets);
  arr_resize(h->temp_buckets, nbuckets);
  arr_resize(h->slots, h->n_total);
  size_t const INVALID_IDX = SIZE_MAX;
  arr_fill(h->slots, INVALID_IDX);
  _hmap_redist_buckets(h->buckets, nbuckets, h->temp_buckets, nbuckets, h->slots);
  h->n_removed = 0;
  {  // Swap the buckets.
    _HashBucket* temp = h->temp_buckets;
    h->temp_buckets   = h->buckets;
    h->buckets        = temp;
  }
}

void _hmap_remove_bin_impl(void*        ptr,
                           size_t const kvsize,
                           void*        keyptr,
                           size_t const keysize)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h) {
    size_t const hash = fnv_hash_bytes(keyptr, keysize);
    Slot const   slot = find_readable_slot_bin(
      hash, h->buckets, h->n_total / HMAP_BUCKET_SIZE, keyptr, keysize, ptr, kvsize);
    switch (slot.type) {
    case EMPTY:  // Should never happen.
      REQUIRE_WITH_MSG(false, "[ERROR] Hash map slots are all full.");
    case INVALID:  // Doesn't contain the key. Do nothing.
      break;
    case OCCUPIED: {
      _HashBucket* bucket      = h->buckets + slot.bucket;
      bucket->hash[slot.slot]  = 1;
      size_t const index       = (size_t)bucket->index[slot.slot];
      bucket->index[slot.slot] = -1;
      REQUIRE_WITH_MSG(slot.bucket * HMAP_BUCKET_SIZE + slot.slot == h->slots[index],
                       "The mapping between items and slots should be consistent.");
      size_t const last = h->n_used - 1;
      if (index != last) {
        memcpy((char*)ptr + index * kvsize, (char*)ptr + last * kvsize, kvsize);
        size_t const s                                               = h->slots[last];
        h->buckets[s / HMAP_BUCKET_SIZE].index[s % HMAP_BUCKET_SIZE] = (ptrdiff_t)index;
        h->slots[index]                                              = s;
      }
      h->slots[last] = SIZE_MAX;
      --h->n_used;
      ++h->n_removed;
      if (h->n_removed > h->n_total / 4) {
        _hmap_compact(ptr);
      }
    }
    };
  }
}

void _hmap_remove_str_impl(void* ptr, size_t const kvsize, char const* key)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h) {
    size_t const hash = fnv_hash_str(key);
    Slot const   slot = find_readable_slot_str(
      hash, h->buckets, h->n_total / HMAP_BUCKET_SIZE, key, ptr, kvsize);
    switch (slot.type) {
    case EMPTY:  // Should never happen.
      REQUIRE_WITH_MSG(false, "[ERROR] Hash map slots are all full.");
    case INVALID:  // Doesn't contain the key. Do nothing.
      break;
    case OCCUPIED: {
      _HashBucket* bucket      = h->buckets + slot.bucket;
      bucket->hash[slot.slot]  = 1;
      size_t const index       = (size_t)bucket->index[slot.slot];
      bucket->index[slot.slot] = -1;
      REQUIRE_WITH_MSG(slot.bucket * HMAP_BUCKET_SIZE + slot.slot == h->slots[index],
                       "The mapping between items and slots should be consistent.");
      size_t const last = h->n_used - 1;
      if (index != last) {
        memcpy((char*)ptr + index * kvsize, (char*)ptr + last * kvsize, kvsize);
        size_t const s                                               = h->slots[last];
        h->buckets[s / HMAP_BUCKET_SIZE].index[s % HMAP_BUCKET_SIZE] = (ptrdiff_t)index;
        h->slots[index]                                              = s;
      }
      h->slots[last] = SIZE_MAX;
      --h->n_used;
      ++h->n_removed;
      if (h->n_removed > h->n_total / 4) {
        _hmap_compact(ptr);
      }
    }
    };
  }
}

// ========== String ==========

Status _str_remove_impl(char* const ptr, size_t const idx)
{
  if (idx < str_len(ptr)) {
    return _arr_remove_impl(ptr, idx, sizeof(char));
  }
  return OUT_OF_BOUNDS;
}

char* _str_push_impl(char* ptr, char val)
{
  size_t newlen = arr_len(ptr) + 1;
  if (newlen < 2) {  // Needs to contain at the very least val and a null terminator.
    newlen = 2;
  }
  arr_resize(ptr, newlen);
  if (ptr == NULL) {
    return NULL;
  }
  char* end = arr_end(ptr);
  *(--end)  = '\0';
  *(--end)  = val;
  return ptr;
}

void str_clear(char* ptr)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h != NULL) {
    h->count = 1;
    ptr[0]   = '\0';
  }
}

char* _str_push_str_impl(char* ptr, char const* tail)
{
  size_t       extra  = strlen(tail);
  size_t const oldlen = str_len(ptr);
  size_t       newlen = oldlen + extra + 1;  // Add 1 for null terminator.
  arr_resize(ptr, newlen);
  if (ptr == NULL) {
    return ptr;
  }
  // Copy chars.
  char*       dst      = ptr + oldlen;
  char const* tail_end = tail + extra;
  while (tail != tail_end) {
    *(dst++) = *(tail++);
  }
  *(dst++) = '\0';
  return ptr;
}

bool str_eq(char* const a, char* const b)
{
  if (a == NULL || b == NULL) {
    return a == b;
  }
  size_t const n = strlen(a);
  if (n != strlen(b)) {
    return false;
  }
  if (n == 0) {
    return true;
  }
  return memcmp(a, b, n) == 0;
}

// ========== String view ==========

StrView sv_from_str(char* str)
{
  char* end = NULL;
  if (str) {
    end = str + strlen(str);
  }
  return (StrView) {.start = str, .end = end};
}

StrView sv_trim_left(StrView sv)
{
  if (sv.start != NULL && sv.end != NULL) {
    while (sv.start < sv.end && isspace((int)(*sv.start))) {
      ++sv.start;
    }
    return sv;
  }
  else {
    return (StrView) {0};
  }
}

StrView sv_trim_right(StrView sv)
{
  if (sv.start != NULL && sv.end != NULL) {
    while (sv.end > sv.start && isspace((int)(*(sv.end - 1)))) {
      --sv.end;
    }
    return sv;
  }
  else {
    return (StrView) {0};
  }
}

StrView sv_split_at_delim(StrView* sv, char const delim)
{
  if (sv->start != NULL && sv->end != NULL) {
    void* mid = memchr(sv->start, delim, sv_len(*sv));
    if (mid != NULL) {
      char* start = sv->start;
      sv->start   = (char*)mid + 1;
      return (StrView) {.start = start, .end = mid};
    }
    else {
      StrView out = *sv;
      sv->start   = NULL;
      sv->end     = NULL;
      return out;
    }
  }
  else {
    return (StrView) {0};
  }
}

bool sv_starts_with(StrView const sv, char const* const prefix)
{
  size_t const n = sv_len(sv);
  if (n == 0 || prefix == NULL) {
    return false;
  }
  size_t const preflen = strlen(prefix);
  if (preflen > n) {
    return false;
  }
  return memcmp(sv.start, prefix, preflen) == 0;
}

bool sv_ends_with(StrView const sv, char const* const suffix)
{
  size_t const n = sv_len(sv);
  if (n == 0 || suffix == NULL) {
    return false;
  }
  size_t const suflen = strlen(suffix);
  if (suflen > n) {
    return false;
  }
  return memcmp(sv.end - suflen, suffix, suflen) == 0;
}

bool sv_contains_str(StrView const sv, char const* const needle)
{
  if (needle == NULL || sv.start == NULL || sv.end == NULL) {
    return false;
  }
  // NOTE: GCC extension has the function `memmem` but writing this out to not depend on
  // the extension.
  size_t const nlen = strlen(needle);
  size_t       slen = sv_len(sv);
  if (nlen > slen || nlen == 0) {
    return false;
  }
  char* ptr = sv.start;
  do {
    char* found = memchr(ptr, *needle, slen);
    if (found == NULL) {
      return false;
    }
    slen -= (size_t)(found - ptr);
    if (nlen > slen) {
      return false;
    }
    if (0 == memcmp(found, needle, nlen)) {
      return true;
    }
    ptr = found + 1;
    --slen;
  } while (true);
}

char* sv_find(StrView sv, char const c)
{
  if (sv.start == NULL || sv.end == NULL) {
    return NULL;
  }
  return memchr(sv.start, c, sv_len(sv));
}

char* sv_rfind(StrView sv, char const c)
{
  if (sv.start != NULL && sv.end != NULL && sv.start < sv.end) {
    char* ptr = sv.end - 1;
    while (ptr > sv.start && *ptr != c) {
      --ptr;
    }
    if (*ptr == c) {
      return ptr;
    }
    else {
      return NULL;
    }
  }
  else {
    return NULL;
  }
}

bool sv_strip_prefix(StrView* sv, char const* const prefix)
{
  if (sv == NULL) {
    return false;
  }
  size_t const n = sv_len(*sv);
  if (n == 0 || prefix == NULL) {
    return false;
  }
  size_t const preflen = strlen(prefix);
  if (preflen > n) {
    return false;
  }
  if (memcmp(sv->start, prefix, preflen) == 0) {
    sv->start += preflen;
    return true;
  }
  else {
    return false;
  }
}

bool sv_strip_suffix(StrView* sv, char const* const suffix)
{
  if (sv == NULL) {
    return false;
  }
  size_t const n = sv_len(*sv);
  if (n == 0 || suffix == NULL) {
    return false;
  }
  size_t const suflen = strlen(suffix);
  if (suflen > n) {
    return false;
  }
  if (memcmp(sv->end - suflen, suffix, suflen) == 0) {
    sv->end -= suflen;
    return true;
  }
  else {
    return false;
  }
}

StrView sv_slice(StrView sv, size_t const start, size_t const end)
{
  if (sv.start == NULL || sv.end == NULL) {
    return sv;
  }
  char* start_new = sv.start + start;
  char* end_new   = sv.start + end;
  if (start_new <= end_new && end_new <= sv.end) {
    sv.start = start_new;
    sv.end   = end_new;
  }
  else {  // Invalid indexing, return empty StrView.
    sv.start = NULL;
    sv.end   = NULL;
  }
  return sv;
}

bool sv_eq(StrView const a, StrView const b)
{
  size_t const n = sv_len(a);
  if (n != sv_len(b)) {
    return false;
  }
  if (n == 0) {
    return true;
  }
  return memcmp(a.start, b.start, n) == 0;
}

// ========== Deck ==========

void _deck_free_impl(void* ptr)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h) {
    arr_free(h->marks);
    arr_free(h->stride_offset);
    arr_free(h->strides);
    arr_free(h->pegs);
    free(h);
  }
}

uint8_t deck_max_depth(void* deck)
{
  _DeckHeader* h = _deck_header(deck);
  if (h != NULL && !arr_is_empty(h->marks)) {
    return h->marks[0].depth + 1;
  }
  return 0;
}

void _deck_push_mark(_DeckHeader* h, uint8_t depth, size_t const pos)
{
  REQUIRE_WITH_MSG(h != NULL, "This should not be called with a null pointer");
  size_t const n_marks = arr_len(h->marks);
  if (n_marks > 0 && depth > h->marks[0].depth) {
    depth = h->marks[0].depth;
  }
  size_t const n_strides = (size_t)depth + 1;
  {  // Make sure we have enough pegs.
    size_t const n_old = arr_len(h->pegs);
    if (n_old < n_strides) {
      arr_resize(h->pegs, n_strides);
      memset(h->pegs + n_old, 0, (n_strides - n_old) * sizeof(*(h->pegs)));
    }
  }
  {  // Update the scan.
    uint64_t last_offset = 0;
    size_t   n           = arr_len(h->stride_offset);
    if (n > 0) {
      last_offset = h->stride_offset[n - 1];
    }
    uint64_t last_depth = 0;
    n                   = arr_len(h->marks);
    if (n > 0) {
      last_depth = (uint64_t)(h->marks[n - 1].depth) + 1;
    }
    size_t const total = last_offset + last_depth;
    arr_push(h->stride_offset, total);
  }
  {  // Update strides.
    size_t const n_old = arr_len(h->strides);
    arr_resize(h->strides, n_old + n_strides);
    memset(h->strides + n_old, 0xff, n_strides * sizeof(*(h->strides)));
    for (size_t i = 0; i < n_strides; ++i) {
      size_t const peg = h->pegs[i];
      if (peg < arr_len(h->marks)) {
        uint64_t*      dst = h->strides + h->stride_offset[peg] + i;
        uint64_t const val = arr_len(h->marks) - peg;
        if (*dst > val) {
          *dst = val;
        }
      }
      h->pegs[i] = arr_len(h->marks);
    }
  }
  Mark const mark = (Mark) {.depth = depth, .pos = pos};
  arr_push(h->marks, mark);
}

void _deck_start_new_arr(_DeckHeader* h, uint8_t depth, size_t const pos)
{
  if (depth == 0)
    return;
  _deck_push_mark(h, depth - 1, pos);
}

void* _deck_push_impl(void* ptr, void* item, size_t const itemsize, uint8_t const depth)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize;
    h                    = malloc(bufsize);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->capacity = 1;
    ptr         = (void*)(h + 1);
  }
  else if (h->count == h->capacity) {
    size_t const newcap = h->capacity * 2;
    h                   = realloc(h, sizeof *h + newcap * itemsize);
    if (h == NULL)
      return NULL;
    h->capacity = newcap;
    ptr         = h + 1;
  }
  _deck_start_new_arr(h, depth, h->count);
  memcpy((char*)ptr + h->count * itemsize, item, itemsize);
  h->count++;
  REQUIRE_WITH_MSG(h->count <= h->capacity, "Count cannot exceed capacity");
  return ptr;
}

void deck_clear(void* ptr)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL)
    return;
  arr_clear(h->marks);
  arr_clear(h->stride_offset);
  arr_clear(h->strides);
  arr_clear(h->pegs);
  h->count = 0;
}

void* _deck_grow_capacity(void* ptr, size_t const itemsize, size_t const n)
{
  // Handle the special case where ptr is NULL and nelems is 0
  // No allocation needed, just return the NULL ptr
  if (ptr == NULL && n == 0) {
    return ptr;  // Return NULL, which is valid for an empty array
  }
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize * n;
    h                    = malloc(bufsize);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->count    = 0;
    h->capacity = n;
    ptr         = h + 1;
  }
  else if (h->capacity < n) {
    h = realloc(h, sizeof *h + n * itemsize);
    if (h == NULL)
      return NULL;
    h->capacity = n;
    ptr         = h + 1;
  }
  if (OK != arr_reserve(h->marks, n)) {
    _deck_free_impl(ptr);
    return NULL;
  }
  if (OK != arr_reserve(h->stride_offset, n)) {
    _deck_free_impl(ptr);
    return NULL;
  }
  return ptr;
}

void deck_flatten(void* ptr)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL) {
    return;
  }
  arr_clear(h->marks);
  arr_clear(h->stride_offset);
  arr_clear(h->strides);
  arr_clear(h->pegs);
  if (h->count > 1) {
    _deck_push_mark(h, 0, 0);
  }
}

void _deck_calc_strides(_DeckHeader* h)
{
  if (h == NULL)
    return;
  arr_clear(h->pegs);
  arr_clear(h->stride_offset);
  arr_clear(h->strides);
  size_t const n_marks = arr_len(h->marks);
  {  // Exclusive scan of depth + 1.
    size_t acc = 0;
    for (size_t i = 0; i < n_marks; ++i) {
      arr_push(h->stride_offset, acc);
      acc += (size_t)(h->marks[i].depth) + 1;
    }
  }
  {  // One stride entry per depth level per mark.
    size_t       n           = arr_len(h->stride_offset);
    size_t const last_offset = n > 0 ? h->stride_offset[n - 1] : 0;
    n                        = arr_len(h->marks);
    size_t const last_depth  = n > 0 ? h->marks[n - 1].depth + 1 : 0;
    size_t const n_total     = last_offset + last_depth;
    size_t const prevlen     = arr_len(h->strides);
    arr_resize(h->strides, n_total);
    memset(h->strides + prevlen, 0xff, (n_total - prevlen) * sizeof(*(h->strides)));
  }
  {  // Fill strides using pegs.
    for (size_t i = 0; i < n_marks; ++i) {
      size_t const d = h->marks[i].depth + 1;
      size_t       n = arr_len(h->pegs);
      if (d > n) {
        arr_resize(h->pegs, d);
        memset(h->pegs + n, 0, (d - n) * sizeof(*(h->pegs)));
      }
      for (size_t j = 0; j < d; ++j) {
        size_t const peg = h->pegs[j];
        if (peg < i) {
          uint64_t* dst = h->strides + h->stride_offset[peg] + j;
          if (*dst > (i - peg)) {
            *dst = i - peg;
          }
        }
        h->pegs[j] = i;
      }
    }
  }
}

void deck_graft(void* ptr)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL)
    return;
  size_t const count     = arr_len(h->marks);
  Mark*        old_marks = h->marks;
  h->marks               = NULL;
  Status const status    = arr_reserve(h->marks, count + h->count);
  REQUIRE_WITH_MSG(status == OK, "Allocation failed.");
  uint64_t     prev    = 0;
  size_t const n_marks = arr_len(old_marks);
  for (size_t i = 0; i < n_marks; ++i) {
    REQUIRE_WITH_MSG(old_marks[i].depth < 255, "Depth cannot exceed 255");
    uint8_t const  new_depth = old_marks[i].depth + 1;
    uint64_t const current   = old_marks[i].pos;
    for (size_t j = prev; j < current; ++j) {
      Mark const mark = {.depth = 0, .pos = j};
      arr_push(h->marks, mark);
    }
    Mark const mark = {.depth = new_depth, .pos = current};
    arr_push(h->marks, mark);
    prev = current + 1;
  }
  size_t const n_items = h->count;
  for (size_t j = prev; j < n_items; ++j) {
    Mark const mark = {.depth = 0, .pos = j};
    arr_push(h->marks, mark);
  }
  arr_free(old_marks);
  _deck_calc_strides(h);
}

void deck_simplify(void* ptr)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL)
    return;
  uint8_t      remap[UINT8_MAX + 1] = {0};
  size_t const d_max                = arr_is_empty(h->marks) ? 0 : h->marks[0].depth;
  {
    Mark* end = arr_end(h->marks);
    for (Mark* m = h->marks; m < end; ++m) {
      remap[m->depth] = 1;
    }
  }
  {  // Do a prefix sum scan to get the remapped depths.
    uint8_t              acc = 0;
    uint8_t const* const end = remap + d_max + 1;
    for (uint8_t* r = remap; r < end; ++r) {
      uint8_t const prev = acc;
      acc += *r;
      *r = prev;
    }
  }
  {  // Replace the mark depth with new values.
    Mark* end = arr_end(h->marks);
    for (Mark* m = h->marks; m < end; ++m) {
      m->depth = remap[m->depth];
    }
  }
  _deck_calc_strides(h);
}
