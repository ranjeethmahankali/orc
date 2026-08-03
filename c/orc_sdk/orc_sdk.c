#include "orc_sdk.h"

#include <assert.h>
#include <ctype.h>
#include <limits.h>
#include <orc_abi.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include <string.h>

static void *_default_alloc(uint64_t const size, uint64_t const alignment)
{
  if (alignment <= ORC_SDK_MALLOC_DEFAULT_ALIGN) {
    return malloc((size_t)size);
  }
  /* Over-allocate: sizeof(void*) bytes for stashing the raw pointer,
     plus (alignment - 1) bytes for worst-case alignment padding. */
  size_t const total = (size_t)size + sizeof(void *) + (size_t)alignment - 1u;
  void        *raw   = malloc(total);
  if (!raw) {
    return NULL;
  }
  uintptr_t addr         = (uintptr_t)raw + sizeof(void *);
  uintptr_t aligned      = (addr + (size_t)alignment - 1u) & ~((size_t)alignment - 1u);
  ((void **)aligned)[-1] = raw;
  return (void *)aligned;
}

static void _default_dealloc(void *ptr, uint64_t const size, uint64_t const alignment)
{
  (void)size;
  if (alignment <= ORC_SDK_MALLOC_DEFAULT_ALIGN) {
    free(ptr);
    return;
  }
  free(((void **)ptr)[-1]);
}

static OrcHost HOST = {
  .abi_version = 0,
  .memory_api  = {.alloc = _default_alloc, .dealloc = _default_dealloc},
  .callbacks   = {0}};
static bool HOST_INIT = false;

void *orc_sdk_alloc(uint64_t const size, uint64_t const alignment)
{
  return HOST.memory_api.alloc(size, alignment);
}

void orc_sdk_free(void *ptr, uint64_t const size, uint64_t const alignment)
{
  HOST.memory_api.dealloc(ptr, size, alignment);
}

void *orc_sdk_realloc(void          *ptr,
                      uint64_t const old_size,
                      uint64_t const new_size,
                      uint64_t const alignment)
{
  if (new_size == 0) {
    HOST.memory_api.dealloc(ptr, old_size, alignment);
    return NULL;
  }
  void *new_ptr = HOST.memory_api.alloc(new_size, alignment);
  if (!new_ptr) {
    return NULL;
  }
  if (ptr) {
    uint64_t const copy_size = old_size < new_size ? old_size : new_size;
    memcpy(new_ptr, ptr, (size_t)copy_size);
    HOST.memory_api.dealloc(ptr, old_size, alignment);
  }
  return new_ptr;
}

void *_orc_sdk_arr_grow(void *ptr, size_t elemsize)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h == NULL) {
    h = orc_sdk_alloc(sizeof *h + elemsize, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    h->count    = 0;
    h->capacity = 1;
    ptr         = h + 1;
  }
  else if (h->count == h->capacity) {
    size_t const newcap   = h->capacity * 2;
    size_t const old_size = sizeof *h + h->capacity * elemsize;
    size_t const new_size = sizeof *h + newcap * elemsize;
    h = orc_sdk_realloc(h, old_size, new_size, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    h->capacity = newcap;
    ptr         = h + 1;
  }
  ORC_SDK_REQUIRE_WITH_MSG(h->count <= h->capacity, "Count cannot exceed capacity");
  return ptr;
}

void *_orc_sdk_arr_grow_capacity(void *ptr, size_t const elemsize, size_t const nelems)
{
  // Handle the special case where ptr is NULL and nelems is 0
  // No allocation needed, just return the NULL ptr
  if (ptr == NULL && nelems == 0) {
    return ptr;  // Return NULL, which is valid for an empty array
  }
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h == NULL) {
    h = orc_sdk_alloc(sizeof *h + elemsize * nelems, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    h->count    = 0;
    h->capacity = nelems;
    ptr         = h + 1;
  }
  else if (h->capacity < nelems) {
    size_t const old_size = sizeof *h + h->capacity * elemsize;
    size_t const new_size = sizeof *h + nelems * elemsize;
    h = orc_sdk_realloc(h, old_size, new_size, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    h->capacity = nelems;
    ptr         = h + 1;
  }
  return ptr;
}

OrcError _orc_sdk_arr_remove_impl(void *ptr, size_t const idx, size_t const elemsize)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h && (idx < h->count)) {
    void        *dst = (char *)ptr + idx * elemsize;
    void        *src = (char *)dst + elemsize;
    size_t const len = --(h->count) - idx;
    if (len)
      memmove(dst, src, len * elemsize);
    return ORC_ERROR_NONE;
  }
  return ORC_ERROR_OUT_OF_BOUNDS;
}

void *_orc_sdk_arr_resize_impl(void *ptr, size_t const elemsize, size_t const count)
{
  size_t const before = orc_sdk_arr_len(ptr);
  if (before < count) {  // Needs to grow.
    ptr = _orc_sdk_arr_grow_capacity(ptr, elemsize, count);
    if (ptr == NULL) {
      return NULL;
    }
    char *dst = (char *)ptr + (before * elemsize);
    memset(dst, 0, (count - before) * elemsize);
  }
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h)
    h->count = count;
  return ptr;
}

void orc_sdk_arr_clear(void *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h)
    h->count = 0;
}

void _orc_sdk_arr_fill_impl(void             *arr,
                            void const *const elem,
                            size_t const      count,
                            size_t const      elemsize)
{
  if (count == 0)
    return;
  char *dst = (char *)arr;
  char *src = (char *)elem;
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
  ORC_SDK_REQUIRE_WITH_MSG(dst == (char *)arr + count * elemsize,
                           "Should have written up to the end of the array.");
}

OrcError _orc_sdk_arr_remove_range_impl(void        *ptr,
                                        size_t const start,
                                        size_t const stop,
                                        size_t const elemsize)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h && (start <= stop) && (start <= h->count) && (stop <= h->count)) {
    if (start == stop || start == h->count) {
      return ORC_ERROR_NONE;
    }
    char        *dst      = (char *)ptr + start * elemsize;
    size_t const nremoved = stop - start;
    size_t const nshift   = h->count - stop;
    if (nshift)
      memmove(dst, dst + nremoved * elemsize, nshift * elemsize);
    h->count -= nremoved;
    return ORC_ERROR_NONE;
  }
  return ORC_ERROR_OUT_OF_BOUNDS;
}

bool orc_sdk_arr_is_empty(void *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  return h == NULL || h->count == 0;
}

// ========== String ==========

OrcError _orc_str_remove_impl(char *const ptr, size_t const idx)
{
  if (idx < orc_str_len(ptr)) {
    return _orc_sdk_arr_remove_impl(ptr, idx, sizeof(char));
  }
  return ORC_ERROR_OUT_OF_BOUNDS;
}

char *_orc_str_push_impl(char *ptr, char val)
{
  size_t newlen = orc_sdk_arr_len(ptr) + 1;
  if (newlen < 2) {  // Needs to contain at the very least val and a null terminator.
    newlen = 2;
  }
  orc_sdk_arr_resize(ptr, newlen);
  if (ptr == NULL) {
    return NULL;
  }
  char *end = orc_sdk_arr_end(ptr);
  *(--end)  = '\0';
  *(--end)  = val;
  return ptr;
}

void orc_str_clear(char *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h != NULL) {
    h->count = 1;
    ptr[0]   = '\0';
  }
}

char *_orc_str_push_str_impl(char *ptr, char const *tail)
{
  size_t       extra  = strlen(tail);
  size_t const oldlen = orc_str_len(ptr);
  size_t       newlen = oldlen + extra + 1;  // Add 1 for null terminator.
  orc_sdk_arr_resize(ptr, newlen);
  if (ptr == NULL) {
    return ptr;
  }
  // Copy chars.
  char       *dst      = ptr + oldlen;
  char const *tail_end = tail + extra;
  while (tail != tail_end) {
    *(dst++) = *(tail++);
  }
  *(dst++) = '\0';
  return ptr;
}

bool orc_str_eq(char *const a, char *const b)
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

OrcStrView orc_sv_from_str(char *str)
{
  char *end = NULL;
  if (str) {
    end = str + strlen(str);
  }
  return (OrcStrView) {.start = str, .end = end};
}

OrcStrView orc_sv_trim_left(OrcStrView sv)
{
  if (sv.start != NULL && sv.end != NULL) {
    while (sv.start < sv.end && isspace((int)(*sv.start))) {
      ++sv.start;
    }
    return sv;
  }
  else {
    return (OrcStrView) {0};
  }
}

OrcStrView orc_sv_trim_right(OrcStrView sv)
{
  if (sv.start != NULL && sv.end != NULL) {
    while (sv.end > sv.start && isspace((int)(*(sv.end - 1)))) {
      --sv.end;
    }
    return sv;
  }
  else {
    return (OrcStrView) {0};
  }
}

OrcStrView orc_sv_split_at_delim(OrcStrView *sv, char const delim)
{
  if (sv->start != NULL && sv->end != NULL) {
    void *mid = memchr(sv->start, delim, orc_sv_len(*sv));
    if (mid != NULL) {
      char *start = sv->start;
      sv->start   = (char *)mid + 1;
      return (OrcStrView) {.start = start, .end = mid};
    }
    else {
      OrcStrView out = *sv;
      sv->start      = NULL;
      sv->end        = NULL;
      return out;
    }
  }
  else {
    return (OrcStrView) {0};
  }
}

bool orc_sv_starts_with(OrcStrView const sv, char const *const prefix)
{
  size_t const n = orc_sv_len(sv);
  if (n == 0 || prefix == NULL) {
    return false;
  }
  size_t const preflen = strlen(prefix);
  if (preflen > n) {
    return false;
  }
  return memcmp(sv.start, prefix, preflen) == 0;
}

bool orc_sv_ends_with(OrcStrView const sv, char const *const suffix)
{
  size_t const n = orc_sv_len(sv);
  if (n == 0 || suffix == NULL) {
    return false;
  }
  size_t const suflen = strlen(suffix);
  if (suflen > n) {
    return false;
  }
  return memcmp(sv.end - suflen, suffix, suflen) == 0;
}

bool orc_sv_contains_str(OrcStrView const sv, char const *const needle)
{
  if (needle == NULL || sv.start == NULL || sv.end == NULL) {
    return false;
  }
  // NOTE: GCC extension has the function `memmem` but writing this out to not depend on
  // the extension.
  size_t const nlen = strlen(needle);
  size_t       slen = orc_sv_len(sv);
  if (nlen > slen || nlen == 0) {
    return false;
  }
  char *ptr = sv.start;
  do {
    char *found = memchr(ptr, *needle, slen);
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

char *orc_sv_find(OrcStrView sv, char const c)
{
  if (sv.start == NULL || sv.end == NULL) {
    return NULL;
  }
  return memchr(sv.start, c, orc_sv_len(sv));
}

char *orc_sv_rfind(OrcStrView sv, char const c)
{
  if (sv.start != NULL && sv.end != NULL && sv.start < sv.end) {
    char *ptr = sv.end - 1;
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

bool orc_sv_strip_prefix(OrcStrView *sv, char const *const prefix)
{
  if (sv == NULL) {
    return false;
  }
  size_t const n = orc_sv_len(*sv);
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

bool orc_sv_strip_suffix(OrcStrView *sv, char const *const suffix)
{
  if (sv == NULL) {
    return false;
  }
  size_t const n = orc_sv_len(*sv);
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

OrcStrView orc_sv_slice(OrcStrView sv, size_t const start, size_t const end)
{
  if (sv.start == NULL || sv.end == NULL) {
    return sv;
  }
  char *start_new = sv.start + start;
  char *end_new   = sv.start + end;
  if (start_new <= end_new && end_new <= sv.end) {
    sv.start = start_new;
    sv.end   = end_new;
  }
  else {  // Invalid indexing, return empty OrcStrView.
    sv.start = NULL;
    sv.end   = NULL;
  }
  return sv;
}

bool orc_sv_eq(OrcStrView const a, OrcStrView const b)
{
  size_t const n = orc_sv_len(a);
  if (n != orc_sv_len(b)) {
    return false;
  }
  if (n == 0) {
    return true;
  }
  return memcmp(a.start, b.start, n) == 0;
}

// ========== Deck ==========

void _orc_sdk_deck_free_impl(void *ptr)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h) {
    orc_sdk_arr_free(h->marks);
    orc_sdk_arr_free(h->stride_offset);
    orc_sdk_arr_free(h->strides);
    orc_sdk_arr_free(h->pegs);
    orc_sdk_free(h, sizeof *h + h->capacity * h->item_size, ORC_SDK_MALLOC_DEFAULT_ALIGN);
  }
}

uint8_t orc_sdk_deck_max_depth(void const *deck)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  if (h != NULL && !orc_sdk_arr_is_empty(h->marks)) {
    return h->marks[0].depth + 1;
  }
  return 0;
}

static void _orc_sdk_deck_push_mark(_OrcSdk_DeckHeader *h,
                                    uint8_t             depth,
                                    size_t const        pos)
{
  ORC_SDK_REQUIRE_WITH_MSG(h != NULL, "This should not be called with a null pointer");
  size_t const n_marks = orc_sdk_arr_len(h->marks);
  if (n_marks > 0 && depth > h->marks[0].depth) {
    depth = h->marks[0].depth;
  }
  size_t const n_strides = (size_t)depth;
  {  // Make sure we have enough pegs.
    size_t const n_old = orc_sdk_arr_len(h->pegs);
    if (n_old < n_strides) {
      orc_sdk_arr_resize(h->pegs, n_strides);
      memset(h->pegs + n_old, 0, (n_strides - n_old) * sizeof(*(h->pegs)));
    }
  }
  {  // Update the scan.
    uint64_t last_offset = 0;
    size_t   n           = orc_sdk_arr_len(h->stride_offset);
    if (n > 0) {
      last_offset = h->stride_offset[n - 1];
    }
    uint64_t last_depth = 0;
    n                   = orc_sdk_arr_len(h->marks);
    if (n > 0) {
      last_depth = (uint64_t)(h->marks[n - 1].depth);
    }
    size_t const total = last_offset + last_depth;
    orc_sdk_arr_push(h->stride_offset, total);
  }
  {  // Update strides.
    size_t const n_old = orc_sdk_arr_len(h->strides);
    orc_sdk_arr_resize(h->strides, n_old + n_strides);
    memset(h->strides + n_old, 0xff, n_strides * sizeof(*(h->strides)));
    for (size_t i = 0; i < n_strides; ++i) {
      size_t const peg = h->pegs[i];
      if (peg < orc_sdk_arr_len(h->marks)) {
        uint64_t      *dst = h->strides + h->stride_offset[peg] + i;
        uint64_t const val = orc_sdk_arr_len(h->marks) - peg;
        if (*dst > val) {
          *dst = val;
        }
      }
      h->pegs[i] = orc_sdk_arr_len(h->marks);
    }
  }
  OrcMark const mark = (OrcMark) {.depth = depth, .pos = pos};
  orc_sdk_arr_push(h->marks, mark);
}

void *_orc_sdk_deck_push_empty(void *ptr, size_t const itemsize, uint8_t const depth)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize;
    h                    = orc_sdk_alloc(bufsize, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->capacity  = 1;
    h->item_size = itemsize;
    ptr          = (void *)(h + 1);
  }
  else if (h->count == h->capacity) {
    size_t const newcap   = h->capacity * 2;
    size_t const old_size = sizeof *h + h->capacity * itemsize;
    size_t const new_size = sizeof *h + newcap * itemsize;
    h = orc_sdk_realloc(h, old_size, new_size, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    h->capacity = newcap;
    ptr         = h + 1;
  }
  if (depth)
    _orc_sdk_deck_push_mark(h, depth - 1, h->count);
  h->count++;
  ORC_SDK_REQUIRE_WITH_MSG(h->count <= h->capacity, "Count cannot exceed capacity");
  return ptr;
}

void *_orc_sdk_deck_push_impl(void         *ptr,
                              void         *item,
                              size_t const  itemsize,
                              uint8_t const depth)
{
  ptr                   = _orc_sdk_deck_push_empty(ptr, itemsize, depth);
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  // Write into the last item we just pushed.
  memcpy((char *)ptr + (h->count - 1) * itemsize, item, itemsize);
  return ptr;
}

void *_orc_sdk_deck_start_new_arr(void *ptr, size_t const itemsize, uint8_t const depth)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize;
    h                    = orc_sdk_alloc(bufsize, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->capacity  = 1;
    h->item_size = itemsize;
    ptr          = (void *)(h + 1);
  }
  if (depth)
    _orc_sdk_deck_push_mark(h, depth - 1, h->count);
  return ptr;
}

void orc_sdk_deck_clear(void const *ptr)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL)
    return;
  orc_sdk_arr_clear(h->marks);
  orc_sdk_arr_clear(h->stride_offset);
  orc_sdk_arr_clear(h->strides);
  orc_sdk_arr_clear(h->pegs);
  h->count = 0;
}

void *_orc_sdk_deck_grow_capacity(void *ptr, size_t const itemsize, size_t const n)
{
  // Handle the special case where ptr is NULL and n is 0 No allocation needed, just
  // return the NULL ptr
  if (ptr == NULL && n == 0) {
    return ptr;  // Return NULL, which is valid for an empty array
  }
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize * n;
    h                    = orc_sdk_alloc(bufsize, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->count     = 0;
    h->capacity  = n;
    h->item_size = itemsize;
    ptr          = h + 1;
  }
  else if (h->capacity < n) {
    size_t const old_size = sizeof *h + h->capacity * itemsize;
    size_t const new_size = sizeof *h + n * itemsize;
    h = orc_sdk_realloc(h, old_size, new_size, ORC_SDK_MALLOC_DEFAULT_ALIGN);
    if (h == NULL)
      return NULL;
    h->capacity = n;
    ptr         = h + 1;
  }
  if (ORC_ERROR_NONE != orc_sdk_arr_reserve(h->marks, n)) {
    _orc_sdk_deck_free_impl(ptr);
    return NULL;
  }
  if (ORC_ERROR_NONE != orc_sdk_arr_reserve(h->stride_offset, n)) {
    _orc_sdk_deck_free_impl(ptr);
    return NULL;
  }
  return ptr;
}

void orc_sdk_deck_flatten(void *ptr)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL) {
    return;
  }
  orc_sdk_arr_clear(h->marks);
  orc_sdk_arr_clear(h->stride_offset);
  orc_sdk_arr_clear(h->strides);
  orc_sdk_arr_clear(h->pegs);
  if (h->count > 1) {
    _orc_sdk_deck_push_mark(h, 0, 0);
  }
}

static void _deck_calc_strides(_OrcSdk_DeckHeader *h)
{
  if (h == NULL)
    return;
  orc_sdk_arr_clear(h->pegs);
  orc_sdk_arr_clear(h->stride_offset);
  orc_sdk_arr_clear(h->strides);
  size_t const n_marks = orc_sdk_arr_len(h->marks);
  {  // Exclusive scan of depth.
    size_t acc = 0;
    for (size_t i = 0; i < n_marks; ++i) {
      orc_sdk_arr_push(h->stride_offset, acc);
      acc += (size_t)(h->marks[i].depth);
    }
  }
  {  // One stride entry per depth level per mark.
    size_t       n           = orc_sdk_arr_len(h->stride_offset);
    size_t const last_offset = n > 0 ? h->stride_offset[n - 1] : 0;
    n                        = orc_sdk_arr_len(h->marks);
    size_t const last_depth  = n > 0 ? h->marks[n - 1].depth : 0;
    size_t const n_total     = last_offset + last_depth;
    size_t const prevlen     = orc_sdk_arr_len(h->strides);
    orc_sdk_arr_resize(h->strides, n_total);
    memset(h->strides + prevlen, 0xff, (n_total - prevlen) * sizeof(*(h->strides)));
  }
  {  // Fill strides using pegs.
    for (size_t i = 0; i < n_marks; ++i) {
      size_t const d = h->marks[i].depth;
      size_t       n = orc_sdk_arr_len(h->pegs);
      if (d > n) {
        orc_sdk_arr_resize(h->pegs, d);
        memset(h->pegs + n, 0, (d - n) * sizeof(*(h->pegs)));
      }
      for (size_t j = 0; j < d; ++j) {
        size_t const peg = h->pegs[j];
        if (peg < i) {
          uint64_t *dst = h->strides + h->stride_offset[peg] + j;
          if (*dst > (i - peg)) {
            *dst = i - peg;
          }
        }
        h->pegs[j] = i;
      }
    }
  }
}

void orc_sdk_deck_graft(void *ptr)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL)
    return;
  size_t const count     = orc_sdk_arr_len(h->marks);
  OrcMark     *old_marks = h->marks;
  h->marks               = NULL;
  OrcError const status  = orc_sdk_arr_reserve(h->marks, count + h->count);
  ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed.");
  uint64_t     prev    = 0;
  size_t const n_marks = orc_sdk_arr_len(old_marks);
  for (size_t i = 0; i < n_marks; ++i) {
    ORC_SDK_REQUIRE_WITH_MSG(old_marks[i].depth < 255, "Depth cannot exceed 255");
    uint8_t const  new_depth = old_marks[i].depth + 1;
    uint64_t const current   = old_marks[i].pos;
    for (size_t j = prev; j < current; ++j) {
      OrcMark const mark = {.depth = 0, .pos = j};
      orc_sdk_arr_push(h->marks, mark);
    }
    OrcMark const mark = {.depth = new_depth, .pos = current};
    orc_sdk_arr_push(h->marks, mark);
    prev = current + 1;
  }
  size_t const n_items = h->count;
  for (size_t j = prev; j < n_items; ++j) {
    OrcMark const mark = {.depth = 0, .pos = j};
    orc_sdk_arr_push(h->marks, mark);
  }
  orc_sdk_arr_free(old_marks);
  _deck_calc_strides(h);
}

void orc_sdk_deck_simplify(void *ptr)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL)
    return;
  uint8_t      remap[UINT8_MAX + 1] = {0};
  size_t const d_max = orc_sdk_arr_is_empty(h->marks) ? 0 : h->marks[0].depth;
  {
    OrcMark *end = orc_sdk_arr_end(h->marks);
    for (OrcMark *m = h->marks; m < end; ++m) {
      remap[m->depth] = 1;
    }
  }
  {  // Do a prefix sum scan to get the remapped depths.
    uint8_t              acc = 0;
    uint8_t const *const end = remap + d_max + 1;
    for (uint8_t *r = remap; r < end; ++r) {
      uint8_t const prev = acc;
      acc += *r;
      *r = prev;
    }
  }
  {  // Replace the mark depth with new values.
    OrcMark *end = orc_sdk_arr_end(h->marks);
    for (OrcMark *m = h->marks; m < end; ++m) {
      m->depth = remap[m->depth];
    }
  }
  _deck_calc_strides(h);
}

char *_orc_sdk_deck_to_str(void        *ptr,
                           size_t const item_size,
                           void (*snprint_item)(void *item, char *dst, size_t len))
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL)
    return NULL;
  char             *output  = NULL;
  size_t const      n_marks = orc_sdk_arr_len(h->marks);
  uint8_t const     dmax    = n_marks == 0 ? 0 : h->marks[0].depth;
  char const *const TAB     = "   ";
  OrcError          status  = ORC_ERROR_NONE;
  for (size_t mi = 0; mi < n_marks; ++mi) {
    size_t next_pos = (mi + 1) < n_marks ? h->marks[mi + 1].pos : h->count;
    if (next_pos > h->count) {
      next_pos = h->count;
    }
    {  // Left padding.
      uint8_t const n_left_pad = dmax - h->marks[mi].depth;
      for (uint8_t i = 0; i < n_left_pad; ++i) {
        status = orc_str_push_str(output, TAB);
        ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      }
    }
    {  // Ruler marking - depth number.
      uint8_t const d_current    = h->marks[mi].depth + 1;
      char          depth_str[8] = {0};
      snprintf(depth_str, 8, "%3d ", d_current);
      status = orc_str_push_str(output, depth_str);
      ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      // Ruler line.
      for (uint8_t i = 0; i < d_current; ++i) {
        status = orc_str_push_str(output, "---");
        ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      }
      status = orc_str_push(output, '|');
      ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
    }
    if (h->marks[mi].pos < next_pos) {  // Items
      // Write the first item without padding.
      size_t pos          = h->marks[mi].pos;
      char  *item         = (char *)ptr + pos * item_size;
      char   item_str[65] = {0};
      snprint_item(item, item_str, 64);
      item_str[64] = '\0';  // Just to be safe.
      status       = orc_str_push(output, ' ');
      ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      status = orc_str_push_str(output, item_str);
      ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      status = orc_str_push(output, '\n');
      ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      // Write remaining items with padding.
      uint8_t const padding = dmax + 1;
      while (++pos < next_pos) {
        item += item_size;
        for (uint8_t i = 0; i < padding; ++i) {
          status = orc_str_push_str(output, TAB);
          ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
        }
        status = orc_str_push_str(output, "    | ");
        ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
        memset(item_str, 0, 65);
        snprint_item(item, item_str, 64);
        item_str[64] = '\0';  // Just to be safe.
        status       = orc_str_push_str(output, item_str);
        ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
        status = orc_str_push(output, '\n');
        ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
      }
    }
    else {
      status = orc_str_push(output, '\n');
      ORC_SDK_REQUIRE_WITH_MSG(status == ORC_ERROR_NONE, "Allocation failed");
    }
  }
  return output;
}

// ========== OrcSdk_DeckView ==========

static size_t _stride(OrcMark const  *marks,
                      uint64_t const *stride_offset,
                      size_t const    n_marks,
                      uint64_t const *strides,
                      size_t const    mark_idx,
                      uint8_t const   depth)
{
  if (mark_idx < n_marks) {
    if (depth == 0) {
      return 1;
    }
    else if (depth > marks[mark_idx].depth) {
      return n_marks - mark_idx;
    }
    else {
      size_t       out   = strides[stride_offset[mark_idx] + depth - 1];
      size_t const other = n_marks - mark_idx;
      if (other < out) {
        out = other;
      }
      return out;
    }
  }
  else {
    return 0;
  }
}

OrcSdk_DeckView _orc_sdk_dv_from_deck_impl(void         *ptr,
                                           size_t const  item_size,
                                           uint8_t const depth)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(ptr);
  if (h == NULL)
    return (OrcSdk_DeckView) {0};
  size_t const n_marks = orc_sdk_arr_len(h->marks);
  return (OrcSdk_DeckView) {
    .items         = ptr,
    .n_items       = h->count,
    .item_size     = item_size,
    .marks         = h->marks,
    .stride_offset = h->stride_offset,
    .n_marks       = n_marks,
    .strides       = h->strides,
    .depth         = depth,
    .start         = 0,
    .end           = depth == 0 ? h->count : n_marks,
  };
}

uint8_t orc_sdk_dv_depth(OrcSdk_DeckView const *const v)
{
  return v->depth;
}

size_t orc_sdk_dv_len(OrcSdk_DeckView const *const v)
{
  if (v->start >= v->end) {
    return 0;
  }
  else if (v->depth == 0) {
    return 1;
  }
  else {
    size_t start_pos = v->n_items;
    if (v->start < v->n_marks) {
      start_pos = v->marks[v->start].pos;
    }
    size_t const next_mark =
      v->start +
      _stride(v->marks, v->stride_offset, v->n_marks, v->strides, v->start, v->depth - 1);
    size_t const end_pos = next_mark < v->n_marks ? v->marks[next_mark].pos : v->n_items;
    return end_pos - start_pos;
  }
}

OrcSdk_DeckView orc_sdk_dv_child(OrcSdk_DeckView const *const v)
{
  if (v->depth == 0) {
    return *v;
  }
  else if (v->depth < 2) {  // Convert from mark indices to item indices.
    size_t start_pos = v->n_items, end_pos = v->n_items;
    if (v->start < v->n_marks) {
      start_pos = v->marks[v->start].pos;
    }
    size_t const next_mark =
      v->start +
      _stride(v->marks, v->stride_offset, v->n_marks, v->strides, v->start, v->depth - 1);
    if (next_mark < v->n_marks) {
      end_pos = v->marks[next_mark].pos;
    }
    return (OrcSdk_DeckView) {.items         = v->items,
                              .n_items       = v->n_items,
                              .item_size     = v->item_size,
                              .marks         = v->marks,
                              .n_marks       = v->n_marks,
                              .stride_offset = v->stride_offset,
                              .strides       = v->strides,
                              .depth         = 0,
                              .start         = start_pos,
                              .end           = end_pos};
  }
  else {
    return (OrcSdk_DeckView) {
      .items         = v->items,
      .n_items       = v->n_items,
      .item_size     = v->item_size,
      .marks         = v->marks,
      .n_marks       = v->n_marks,
      .stride_offset = v->stride_offset,
      .strides       = v->strides,
      .depth         = v->depth - 1,
      .start         = v->start,
      .end =
        v->start +
        _stride(
          v->marks, v->stride_offset, v->n_marks, v->strides, v->start, v->depth - 1)};
  }
}

bool orc_sdk_dv_advance(OrcSdk_DeckView *const v)
{
  if (v->start >= v->end)
    return false;
  size_t new_start = v->start;
  if (v->depth == 0) {
    ++new_start;
  }
  else {
    new_start +=
      _stride(v->marks, v->stride_offset, v->n_marks, v->strides, v->start, v->depth - 1);
  }
  if (new_start < v->end) {
    v->start = new_start;
    return true;
  }
  return false;
}

void const *orc_sdk_dv_item_ptr(OrcSdk_DeckView const *const v)
{
  size_t const start_pos = v->depth == 0           ? v->start
                           : v->start < v->n_marks ? v->marks[v->start].pos
                                                   : v->n_items;
  return (char *)v->items + start_pos * v->item_size;
}

// ========== OrcSdk_DeckWriter ==========

uint8_t _orc_sdk_dw_next_depth(OrcSdk_DeckWriter *writer)
{
  assert(writer != NULL);
  if (writer->has_next_depth) {
    writer->has_next_depth = false;
    return writer->next_depth;
  }
  else {
    return 0;
  }
}

OrcSdk_DeckWriter orc_sdk_dw_child(OrcSdk_DeckWriter *writer)
{
  ORC_SDK_REQUIRE_WITH_MSG(writer != NULL && writer->depth > 0,
                           "Cannot create a child for this writer");
  uint8_t depth = writer->depth - 1;
  if (writer->has_next_depth) {
    depth                  = writer->next_depth;
    writer->has_next_depth = false;
  }
  return (OrcSdk_DeckWriter) {
    .deck           = writer->deck,
    .item_size      = writer->item_size,
    .depth          = writer->depth - 1,
    .has_next_depth = true,
    .next_depth     = depth,
    .start          = orc_sdk_deck_len(*(writer->deck)),
  };
}

OrcError _orc_sdk_dw_push_impl(OrcSdk_DeckWriter *writer, void *item)
{
  *(writer->deck) = _orc_sdk_deck_push_impl(
    *(writer->deck), item, writer->item_size, _orc_sdk_dw_next_depth(writer));
  return *(writer->deck) == NULL ? ORC_ERROR_ALLOC_FAILED : ORC_ERROR_NONE;
}

OrcError orc_sdk_dw_close(OrcSdk_DeckWriter *writer)
{
  OrcError status = ORC_ERROR_NONE;
  if (writer != NULL && writer->has_next_depth) {
    *(writer->deck) =
      _orc_sdk_deck_start_new_arr(*(writer->deck), writer->item_size, writer->next_depth);
    if (*(writer->deck) == NULL) {
      status = ORC_ERROR_ALLOC_FAILED;
    }
    *writer = (OrcSdk_DeckWriter) {0};
  }
  return status;
}

OrcError orc_sdk_dw_advance(OrcSdk_DeckWriter *writer)
{
  if (writer == NULL)
    return ORC_ERROR_NULL_PTR;
  if (writer->has_next_depth) {
    *(writer->deck) =
      _orc_sdk_deck_start_new_arr(*(writer->deck), writer->item_size, writer->next_depth);
    if (*(writer->deck) == NULL) {
      return ORC_ERROR_ALLOC_FAILED;
    }
  }
  writer->has_next_depth = true;
  writer->next_depth     = writer->depth;
  writer->start          = orc_sdk_deck_len(*(writer->deck));
  return ORC_ERROR_NONE;
}

void *orc_sdk_dw_push_empty(OrcSdk_DeckWriter *writer)
{
  *(writer->deck) = _orc_sdk_deck_push_empty(
    *(writer->deck), writer->item_size, _orc_sdk_dw_next_depth(writer));
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(*(writer->deck));
  if (h == NULL) {
    return NULL;
  }
  void *ptr = (char *)(*(writer->deck)) + (h->count - 1) * writer->item_size;
  memset(ptr, 0, writer->item_size);
  return ptr;
}

// ========== Dims (Units) ==========

bool orc_sdk_dims_equal(OrcDims const a, OrcDims const b)
{
  return memcmp(a, b, sizeof(*a) * ORC_NUM_DIMS) == 0;
}

void orc_sdk_dims_multiply(OrcDims const a, OrcDims const b, OrcDims out)
{
  for (size_t i = 0; i < ORC_NUM_DIMS; ++i) {
    out[i] = a[i] + b[i];
  }
}

void orc_sdk_dims_divide(OrcDims const a, OrcDims const b, OrcDims out)
{
  for (size_t i = 0; i < ORC_NUM_DIMS; ++i) {
    out[i] = a[i] - b[i];
  }
}

void orc_sdk_dims_pow(OrcDims const a, int const pow, OrcDims out)
{
  for (size_t i = 0; i < ORC_NUM_DIMS; ++i) {
    out[i] = a[i] * pow;
  }
}

// ========== Combinations ==========

OrcSdk_DeckView _dv_from_oh(OrcHandle const *const handle, uint8_t const depth)
{
  if (handle == NULL)
    return (OrcSdk_DeckView) {0};
  else
    return (OrcSdk_DeckView) {
      .items         = handle->items,
      .n_items       = (size_t)handle->n_items,
      .item_size     = (size_t)handle->item_size,
      .marks         = handle->marks,
      .stride_offset = handle->stride_offset,
      .n_marks       = (size_t)handle->n_marks,
      .strides       = handle->strides,
      .depth         = depth,
      .start         = 0,
      .end           = (size_t)(depth == 0 ? handle->n_items : handle->n_marks),
    };
}

typedef struct
{
  OrcSdk_DeckView   *view_matrix;
  uint8_t           *input_depths;
  size_t             n_inputs;
  OrcSdk_DeckWriter *writer_matrix;
  uint8_t           *output_depths;
  size_t             n_outputs;
  size_t             stack_depth;
} Combinations;

void orc_sdk_comb_free(void *ptr)
{
  if (ptr == NULL)
    return;
  Combinations *comb = (Combinations *)ptr;
  ORC_SDK_REQUIRE_WITH_MSG(comb->n_inputs == orc_sdk_arr_len(comb->input_depths) &&
                             comb->n_outputs == orc_sdk_arr_len(comb->output_depths),
                           "Invalid combinations");
  orc_sdk_arr_free(comb->view_matrix);
  orc_sdk_arr_free(comb->input_depths);
  orc_sdk_arr_free(comb->writer_matrix);
  orc_sdk_arr_free(comb->output_depths);
  orc_sdk_free(comb, sizeof(Combinations), ORC_SDK_MALLOC_DEFAULT_ALIGN);
}

uint8_t _oh_max_depth(OrcHandle const *handle)
{
  if (handle != NULL && handle->n_marks != 0) {
    return handle->marks[0].depth + 1;
  }
  return 0;
}

static OrcSdkTypeCallbacksGetterFn PLUGIN_TYPE_FN = NULL;

void orc_sdk_init(OrcHost const *host, OrcSdkTypeCallbacksGetterFn type_fn)
{
  if (host) {
    ORC_SDK_REQUIRE_WITH_MSG(
      host->abi_version == ORC_ABI_VERSION,
      "The host's ABI version doesn't match what this plugin was compiled with. The "
      "plugin should have checked the ABI version of the host before calling "
      "orc_sdk_init. This should never happen.");
    HOST.abi_version = host->abi_version;
    if (host->memory_api.alloc != NULL && host->memory_api.dealloc != NULL) {
      HOST.memory_api = host->memory_api;
    }
    if (host->callbacks.report_progress) {
      HOST.callbacks.report_progress = host->callbacks.report_progress;
    }
    if (host->callbacks.report_message) {
      HOST.callbacks.report_message = host->callbacks.report_message;
    }
    if (host->callbacks.check_cancellation) {
      HOST.callbacks.check_cancellation = host->callbacks.check_cancellation;
    }
    if (host->callbacks.report_intermediate_output) {
      HOST.callbacks.report_intermediate_output =
        host->callbacks.report_intermediate_output;
    }
    HOST_INIT = true;
  }
  PLUGIN_TYPE_FN = type_fn;
}

bool _is_type_info_valid(OrcSdkTypeInfo const *info)
{
  return info->copy_fn != NULL && info->item_size != 0;
}

OrcError _oh_free_fn(OrcHandle *const handle)
{
  if (handle == NULL) {
    return ORC_ERROR_NONE;
  }
  ORC_SDK_REQUIRE_WITH_MSG(handle->handle == (uint64_t)handle->items,
                           "In this implementation the handle is just the pointer.");
  ItemFreeFn free_fn = NULL;
  switch (handle->type_id) {
  case ORC_TYPE_U8:
  case ORC_TYPE_U16:
  case ORC_TYPE_U32:
  case ORC_TYPE_U64:
    // Scalars.
  case ORC_TYPE_F32:
  case ORC_TYPE_F64:
    // Signed integers.
  case ORC_TYPE_I8:
  case ORC_TYPE_I16:
  case ORC_TYPE_I32:
  case ORC_TYPE_I64:
    // Proxy for an item in a tree.
  case ORC_TYPE_PROXY:
    break;
  default:
    if (PLUGIN_TYPE_FN) {
      OrcSdkTypeInfo info = PLUGIN_TYPE_FN(handle->type_id);
      if (!_is_type_info_valid(&info)) {
        return ORC_ERROR_TYPE_MISMATCH;
      }
      free_fn = info.free_fn;
    }
    else {
      return ORC_ERROR_TYPE_MISMATCH;
    }
    break;
  }
  if (free_fn) {
    // Free the individual items from the deck before freeing the Deck container itself.
    size_t const count = orc_sdk_deck_len(handle->items);
    char        *data  = (char *)handle->items;
    for (size_t i = 0; i < count; ++i, data += handle->item_size) {
      free_fn(data);
    }
  }
  _orc_sdk_deck_free_impl((void *)handle->items);  // Now we can free the deck container.
  memset(handle, 0, sizeof(OrcHandle));
  return ORC_ERROR_NONE;
}

void orc_sdk_oh_update(OrcHandle *handle)
{
  ORC_SDK_REQUIRE_WITH_MSG(handle != NULL, "Invalid handle");
  ORC_SDK_REQUIRE_WITH_MSG(handle->type_id != 0, "Invalid type id");
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(handle->items);
  handle->handle        = (uint64_t)handle->items;
  handle->n_items       = h->count;
  handle->item_size     = h->item_size;
  handle->marks         = h->marks;
  handle->stride_offset = h->stride_offset;
  handle->n_marks       = orc_sdk_arr_len(h->marks);
  handle->strides       = h->strides;
  handle->free_fn       = _oh_free_fn;
}

OrcError orc_sdk_oh_ensure_alloc(OrcTypeId const type_id, OrcHandle *handle)
{
  if ((handle->handle == 0) != (handle->items == NULL)) {
    return ORC_ERROR_INVALID_HANDLE;
  }
  if (handle->items == NULL) {
    return orc_sdk_handle_alloc(type_id, handle);
  }
  // The deck is already allocated. If the type doesn't match, we free it and reallocate.
  if (handle->type_id != type_id) {
    OrcError const err = orc_sdk_handle_free(handle);
    if (err) {
      return err;
    }
    return orc_sdk_handle_alloc(type_id, handle);
  }
  // No need to allocate. Handle already points to an allocated deck, of the correct type.
  return ORC_ERROR_NONE;
}

void *orc_sdk_comb_init(OrcHandle const **inputs,
                        uint8_t const    *input_depths,
                        size_t const      n_inputs,
                        OrcHandle       **outputs,
                        uint8_t const    *output_depths,
                        size_t const      n_outputs)
{
  if (inputs == NULL || outputs == NULL || input_depths == NULL ||
      output_depths == NULL) {
    return NULL;
  }
  uint8_t max_delta = 0;
  for (size_t i = 0; i < n_inputs; ++i) {
    uint8_t const input_depth = _oh_max_depth(inputs[i]);
    uint8_t const arg_depth   = input_depths[i];
    if (input_depth > arg_depth) {
      uint8_t const delta = input_depth - arg_depth;
      if (delta > max_delta) {
        max_delta = delta;
      }
    }
  }
  // Initialize output combinations structure.
  size_t const  stack_depth = max_delta + 1;
  Combinations *out = orc_sdk_alloc(sizeof(Combinations), ORC_SDK_MALLOC_DEFAULT_ALIGN);
  memset(out, 0, sizeof(Combinations));
  // Allocate buffers.
  orc_sdk_arr_resize(out->view_matrix, n_inputs * stack_depth);
  orc_sdk_arr_resize(out->writer_matrix, n_outputs * stack_depth);
  orc_sdk_arr_resize(out->input_depths, n_inputs);
  orc_sdk_arr_resize(out->output_depths, n_outputs);
  ORC_SDK_REQUIRE_WITH_MSG(out->view_matrix != NULL && out->writer_matrix != NULL &&
                             out->input_depths != NULL && out->output_depths != NULL,
                           "Allocation failed");
  // Populate input views and depths.
  for (size_t i = 0; i < n_inputs; ++i) {
    uint8_t const arg_depth = input_depths[i];
    out->input_depths[i]    = arg_depth;
    OrcSdk_DeckView *dst    = out->view_matrix + i * stack_depth;
    // The first view.
    *dst = _dv_from_oh(inputs[i], arg_depth + max_delta);
    // Telescope the views until we reach the target depth.
    for (size_t d = 1; d < stack_depth; ++d) {
      OrcSdk_DeckView child = orc_sdk_dv_child(dst);
      *(++dst)              = child;
    }
    ORC_SDK_REQUIRE(dst->depth == arg_depth);
  }
  // Populate output decks, writers, and depths.
  for (size_t i = 0; i < n_outputs; ++i) {
    uint8_t const arg_depth = output_depths[i];
    out->output_depths[i]   = arg_depth;
    OrcSdk_DeckWriter *dst  = out->writer_matrix + i * stack_depth;
    // The first writer.
    *dst = (OrcSdk_DeckWriter) {
      .deck           = (void **)&(outputs[i]->items),
      .item_size      = outputs[i]->item_size,
      .depth          = arg_depth + max_delta,
      .has_next_depth = true,
      .next_depth     = arg_depth + max_delta,
      .start          = orc_sdk_deck_len(outputs[i]->items),
    };
    for (size_t d = 1; d < stack_depth; ++d) {
      OrcSdk_DeckWriter child = orc_sdk_dw_child(dst);
      *(++dst)                = child;
    }
    ORC_SDK_REQUIRE(dst->depth == arg_depth);
  }
  out->n_inputs    = n_inputs;
  out->n_outputs   = n_outputs;
  out->stack_depth = stack_depth;
  return out;
}

void *orc_sdk_comb_advance(void *ptr)
{
  if (ptr == NULL)
    return NULL;
  Combinations *comb      = (Combinations *)ptr;
  size_t const  n_inputs  = comb->n_inputs;
  size_t const  n_outputs = comb->n_outputs;
  {  // Try to advance all the input views and check if at least one input advances.
    bool any_advanced = false;
    for (size_t i = 0; i < n_inputs; ++i) {
      OrcSdk_DeckView *last_view = comb->view_matrix + (i + 1) * comb->stack_depth - 1;
      bool const       advanced  = orc_sdk_dv_advance(last_view);
      any_advanced               = any_advanced || advanced;
    }
    if (any_advanced) {  // At least one input advanced. Advance all the writers.
      for (size_t i = 0; i < n_outputs; ++i) {
        OrcSdk_DeckWriter *last_writer =
          comb->writer_matrix + (i + 1) * comb->stack_depth - 1;
        OrcError const status = orc_sdk_dw_advance(last_writer);
        ORC_SDK_REQUIRE(status == ORC_ERROR_NONE);
      }
      return comb;
    }
  }
  enum
  {
    CONTINUE,
    ADVANCED,
    EXHAUSTED
  } state = CONTINUE;
  ORC_SDK_REQUIRE(comb->stack_depth > 0);
  size_t stack_top = comb->stack_depth - 1;
  do {
    if (stack_top == 0) {
      state = EXHAUSTED;
      break;
    }
    for (size_t i = 0; i < n_inputs; ++i) {  // Pop all the inputs.
      OrcSdk_DeckView *last_view = comb->view_matrix + i * comb->stack_depth + stack_top;
      *last_view                 = (OrcSdk_DeckView) {0};
    }
    for (size_t i = 0; i < n_outputs; ++i) {  // Pop all the outputs.
      OrcSdk_DeckWriter *last_writer =
        comb->writer_matrix + i * comb->stack_depth + stack_top;
      OrcError const status = orc_sdk_dw_close(last_writer);
      ORC_SDK_REQUIRE(status == ORC_ERROR_NONE);
    }
    --stack_top;
    // Try to advance lower in the stack.
    for (size_t i = 0; i < n_inputs; ++i) {  // Advance all the inputs.
      OrcSdk_DeckView *last_view = comb->view_matrix + i * comb->stack_depth + stack_top;
      if (orc_sdk_dv_advance(last_view)) {
        state = ADVANCED;
      }
    }
    if (state == ADVANCED) {  // Only advance all the outputs if inputs did.
      for (size_t i = 0; i < n_outputs; ++i) {
        OrcSdk_DeckWriter *last_writer =
          comb->writer_matrix + i * comb->stack_depth + stack_top;
        OrcError const status = orc_sdk_dw_advance(last_writer);
        ORC_SDK_REQUIRE(status == ORC_ERROR_NONE);
      }
    }
  } while (state == CONTINUE);
  switch (state) {
  case ADVANCED: {
    // Telescope the inputs to the desired depth.
    for (size_t i = 0; i < n_inputs; ++i) {
      OrcSdk_DeckView *prev = comb->view_matrix + i * comb->stack_depth + stack_top;
      for (size_t d = stack_top + 1; d < comb->stack_depth; ++d) {
        OrcSdk_DeckView *dst = prev + 1;
        *dst                 = orc_sdk_dv_child(prev);
        prev                 = dst;
      }
    }
    // Telescope the outputs to the desired depth.
    for (size_t i = 0; i < n_outputs; ++i) {
      OrcSdk_DeckWriter *prev = comb->writer_matrix + i * comb->stack_depth + stack_top;
      for (size_t d = stack_top + 1; d < comb->stack_depth; ++d) {
        OrcSdk_DeckWriter *dst = prev + 1;
        *dst                   = orc_sdk_dw_child(prev);
        prev                   = dst;
      }
    }
    return comb;
  } break;
  case EXHAUSTED: {
    orc_sdk_comb_free(comb);
    return NULL;
  }
  case CONTINUE:  // fall through.
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false,
      "Previous loop can only terminate when either we advance, or exhaust the inputs.");
    return NULL;
  }
}

OrcSdk_DeckView orc_sdk_comb_get_input(void *ptr, size_t const index)
{
  ORC_SDK_REQUIRE_WITH_MSG(ptr != NULL, "Invalid combinations");
  Combinations *comb = (Combinations *)ptr;
  ORC_SDK_REQUIRE_WITH_MSG(index < comb->n_inputs, "Index out of bounds");
  return *(comb->view_matrix + (index + 1) * comb->stack_depth - 1);
}

OrcSdk_DeckWriter *orc_sdk_comb_get_output(void *ptr, size_t const index)
{
  ORC_SDK_REQUIRE_WITH_MSG(ptr != NULL, "Invalid combinations");
  Combinations *comb = (Combinations *)ptr;
  ORC_SDK_REQUIRE_WITH_MSG(index < comb->n_outputs, "Index out of bounds");
  return comb->writer_matrix + (index + 1) * comb->stack_depth - 1;
}

// ========== FFI helper functions ==========

OrcError orc_sdk_handle_alloc(OrcTypeId const id, OrcHandle *const out)
{
  if (out == NULL) {
    return ORC_ERROR_INVALID_HANDLE;
  }
  out->item_size = 0;
  switch (id) {
    // Unsigned integers.
  case ORC_TYPE_U8:
    out->item_size = sizeof(uint8_t);
    break;
  case ORC_TYPE_U16:
    out->item_size = sizeof(uint16_t);
    break;
  case ORC_TYPE_U32:
    out->item_size = sizeof(uint32_t);
    break;
  case ORC_TYPE_U64:
    out->item_size = sizeof(uint64_t);
    break;
    // Scalars.
  case ORC_TYPE_F32:
    out->item_size = sizeof(float);
    break;
  case ORC_TYPE_F64:
    out->item_size = sizeof(double);
    break;
    // Signed integers.
  case ORC_TYPE_I8:
    out->item_size = sizeof(int8_t);
    break;
  case ORC_TYPE_I16:
    out->item_size = sizeof(int16_t);
    break;
  case ORC_TYPE_I32:
    out->item_size = sizeof(int32_t);
    break;
  case ORC_TYPE_I64:
    out->item_size = sizeof(int64_t);
    break;
  default: {
    if (PLUGIN_TYPE_FN) {
      OrcSdkTypeInfo info = PLUGIN_TYPE_FN(id);
      if (!_is_type_info_valid(&info)) {
        return ORC_ERROR_TYPE_MISMATCH;
      }
      out->item_size = info.item_size;
    }
    else {
      return ORC_ERROR_TYPE_MISMATCH;
    }
  }
  }
  ORC_SDK_REQUIRE_WITH_MSG(out->item_size != 0,
                           "Item size cannot be inferred from the type id.");
  out->type_id           = id;
  size_t const INIT_SIZE = 1;
  void        *deck_ptr  = _orc_sdk_deck_grow_capacity(NULL, out->item_size, INIT_SIZE);
  _OrcSdk_DeckHeader *h  = _orc_sdk_deck_header(deck_ptr);
  // Assign to the output deck.
  out->handle  = (uint64_t)deck_ptr;
  out->items   = deck_ptr;
  out->free_fn = _oh_free_fn;
  ORC_SDK_REQUIRE_WITH_MSG(h->count == 0, "New deck must be empty");
  out->n_items         = h->count;
  out->marks           = h->marks;
  out->stride_offset   = h->stride_offset;
  size_t const n_marks = orc_sdk_arr_len(h->marks);
  ORC_SDK_REQUIRE_WITH_MSG(n_marks == 0, "New deck must have no marks");
  out->n_marks = n_marks;
  out->strides = h->strides;
  return ORC_ERROR_NONE;
}

OrcError orc_sdk_handle_free(OrcHandle *const handle)
{
  if (handle == NULL) {
    return ORC_ERROR_NONE;
  }
  if (handle->free_fn == NULL) {
    return ORC_ERROR_INVALID_HANDLE;
  }
  return handle->free_fn(handle);
}

#define DEFINE_TRIVIAL_COPY_FN(suffix, type)                                         \
  static void _copy_items_##suffix(void const *src, void *dst, size_t const n_items) \
  {                                                                                  \
    memcpy(dst, src, n_items * sizeof(type));                                        \
  }

DEFINE_TRIVIAL_COPY_FN(u8, uint8_t)
DEFINE_TRIVIAL_COPY_FN(u16, uint16_t)
DEFINE_TRIVIAL_COPY_FN(u32, uint32_t)
DEFINE_TRIVIAL_COPY_FN(u64, uint64_t)
DEFINE_TRIVIAL_COPY_FN(f32, float)
DEFINE_TRIVIAL_COPY_FN(f64, double)
DEFINE_TRIVIAL_COPY_FN(i8, int8_t)
DEFINE_TRIVIAL_COPY_FN(i16, int16_t)
DEFINE_TRIVIAL_COPY_FN(i32, int32_t)
DEFINE_TRIVIAL_COPY_FN(i64, int64_t)
DEFINE_TRIVIAL_COPY_FN(proxy, OrcItemProxy)

OrcError _copy_items(OrcTypeId const type_id,
                     void const     *src,
                     void           *dst,
                     size_t const    n_items)
{
  CopyItemsFn copy_fn = NULL;
  switch (type_id) {
  case ORC_TYPE_U8:
    copy_fn = _copy_items_u8;
    break;
  case ORC_TYPE_U16:
    copy_fn = _copy_items_u16;
    break;
  case ORC_TYPE_U32:
    copy_fn = _copy_items_u32;
    break;
  case ORC_TYPE_U64:
    copy_fn = _copy_items_u64;
    break;
    // Scalars.
  case ORC_TYPE_F32:
    copy_fn = _copy_items_f32;
    break;
  case ORC_TYPE_F64:
    copy_fn = _copy_items_f64;
    break;
    // Signed integers.
  case ORC_TYPE_I8:
    copy_fn = _copy_items_i8;
    break;
  case ORC_TYPE_I16:
    copy_fn = _copy_items_i16;
    break;
  case ORC_TYPE_I32:
    copy_fn = _copy_items_i32;
    break;
  case ORC_TYPE_I64:
    copy_fn = _copy_items_i64;
    break;
    // Proxy for an item in a tree.
  case ORC_TYPE_PROXY:
    copy_fn = _copy_items_proxy;
    break;
  default:
    if (PLUGIN_TYPE_FN) {
      OrcSdkTypeInfo info = PLUGIN_TYPE_FN(type_id);
      if (!_is_type_info_valid(&info)) {
        return ORC_ERROR_TYPE_MISMATCH;
      }
      copy_fn = info.copy_fn;
    }
    else {
      return ORC_ERROR_TYPE_MISMATCH;
    }
    break;
  }
  copy_fn(src, dst, n_items);
  return ORC_ERROR_NONE;
}

OrcError orc_sdk_deck_from_proxy(OrcHandle const   *inputs,
                                 uint64_t const     n_inputs,
                                 OrcProxyType const proxy_type,
                                 OrcHandle const   *proxy,
                                 OrcHandle         *out)
{
  if (n_inputs == 0) {
    return ORC_ERROR_NONE;  // Nothing to do.
  }
  if (proxy->type_id != ORC_TYPE_PROXY) {
    // Invalid proxy deck
    return ORC_ERROR_INVALID_PROXY;
  }
  OrcTypeId const id        = inputs[0].type_id;
  size_t const    item_size = inputs[0].item_size;
  for (size_t i = 1; i < n_inputs; ++i) {
    if (id != inputs[i].type_id) {
      // All input decks must be of the same type
      return ORC_ERROR_TYPE_MISMATCH;
    }
  }
  OrcError const err = orc_sdk_handle_alloc(id, out);
  if (err != ORC_ERROR_NONE) {
    return err;
  }
  ORC_SDK_REQUIRE_WITH_MSG(out->handle == (uint64_t)out->items,
                           "In this implementation the handle is just the pointer.");
  switch (proxy_type) {
  case ORC_DECK_PROXY_COPY_ALL: {
    if (n_inputs != 1) {
      // COPY_ALL is only valid with a single input.
      orc_sdk_handle_free(out);
      return ORC_ERROR_INVALID_PROXY;
    }
    size_t const n_items = inputs[0].n_items;
    void *deck = _orc_sdk_deck_grow_capacity((void *)out->items, item_size, n_items);
    _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
    h->item_size          = item_size;
    memcpy(out->dims, inputs[0].dims, sizeof(OrcDims));
    out->type_id = id;
    {  // Copy the data.
      memset(deck, 0, item_size * n_items);
      OrcError const e = _copy_items(id, inputs[0].items, deck, n_items);
      if (e) {
        return e;
      }
    }
    h->count = n_items;
    // Copy the marks.
    size_t const n_marks = inputs[0].n_marks;
    h->marks             = _orc_sdk_arr_resize_impl(h->marks, sizeof(OrcMark), n_marks);
    memcpy(h->marks, inputs[0].marks, sizeof(OrcMark) * n_marks);
    _deck_calc_strides(h);
    // Assign the pointer back to the handle and update the data inside the handle.
    out->items = deck;
    orc_sdk_oh_update(out);
  } break;
  case ORC_DECK_PROXY_COPY_ITEMS: {
    if (n_inputs != 1) {
      // COPY_ITEMS is only valid with a single input.
      orc_sdk_handle_free(out);
      return ORC_ERROR_INVALID_PROXY;
    }
    size_t const n_items = inputs[0].n_items;
    void *deck = _orc_sdk_deck_grow_capacity((void *)out->items, item_size, n_items);
    _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
    h->item_size          = item_size;
    memcpy(out->dims, inputs[0].dims, sizeof(OrcDims));
    out->type_id = id;
    {  // Copy the data.
      memset(deck, 0, item_size * n_items);
      OrcError const e = _copy_items(id, inputs[0].items, deck, n_items);
      if (e) {
        return e;
      }
    }
    h->count = n_items;
    // Copy the marks from the proxy, NOT the input.
    size_t const n_marks = proxy->n_marks;
    h->marks             = _orc_sdk_arr_resize_impl(h->marks, sizeof(OrcMark), n_marks);
    memcpy(h->marks, proxy->marks, sizeof(OrcMark) * n_marks);
    _deck_calc_strides(h);
    // Update the handle pointer and update the rest of the data inside the handle.
    out->items = deck;
    orc_sdk_oh_update(out);
  } break;
  case ORC_DECK_PROXY_SHUFFLE: {
    size_t const n_items = proxy->n_items;
    void *deck = _orc_sdk_deck_grow_capacity((void *)out->items, item_size, n_items);
    _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
    h->item_size          = item_size;
    memcpy(out->dims, proxy->dims, sizeof(OrcDims));
    out->type_id = id;
    // Copy the data one at a time from by iterating over the proxy.
    OrcItemProxy *proxies = (OrcItemProxy *)proxy->items;
    while (h->count < n_items) {
      ORC_SDK_REQUIRE_WITH_MSG(
        h->count < proxy->n_items && proxies[h->count].tree < n_inputs,
        "Index out of bounds");
      void *src =
        (char *)inputs[proxies[h->count].tree].items + item_size * proxies[h->count].item;
      void          *dst = (char *)deck + item_size * h->count;
      OrcError const e   = _copy_items(id, src, dst, 1);
      if (e) {
        return e;
      }
      ++h->count;
    }
    // Copy the marks from the proxy, NOT the input.
    size_t const n_marks = proxy->n_marks;
    h->marks             = _orc_sdk_arr_resize_impl(h->marks, sizeof(OrcMark), n_marks);
    memcpy(h->marks, proxy->marks, sizeof(OrcMark) * n_marks);
    _deck_calc_strides(h);
    // Update the handle pointer and update the rest of the data inside the handle.
    out->items = deck;
    orc_sdk_oh_update(out);
  } break;
  default:
    orc_sdk_handle_free(out);
    return ORC_ERROR_INVALID_PROXY;
  }
  return ORC_ERROR_NONE;
}
