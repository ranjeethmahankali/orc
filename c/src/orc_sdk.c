#include "orc_sdk.h"

#include <assert.h>
#include <ctype.h>
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int stat_printf(Status const s)
{
  switch (s) {
  case OK:
    return printf("[OK]");
  case ALLOC_FAILED:
    return printf("[ERROR] Failed to allocate memory");
  case OUT_OF_BOUNDS:
    return printf("[ERROR] Index is out of bounds");
  default:  // Weird.
    return -1;
  }
}

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

static void _deck_push_mark(_DeckHeader* h, uint8_t depth, size_t const pos)
{
  REQUIRE_WITH_MSG(h != NULL, "This should not be called with a null pointer");
  size_t const n_marks = arr_len(h->marks);
  if (n_marks > 0 && depth > h->marks[0].depth) {
    depth = h->marks[0].depth;
  }
  size_t const n_strides = (size_t)depth;
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
      last_depth = (uint64_t)(h->marks[n - 1].depth);
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
  OrcMark const mark = (OrcMark) {.depth = depth, .pos = pos};
  arr_push(h->marks, mark);
}

void* _deck_push_empty(void* ptr, size_t const itemsize, uint8_t const depth)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize;
    h                    = malloc(bufsize);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->capacity  = 1;
    h->item_size = itemsize;
    ptr          = (void*)(h + 1);
  }
  else if (h->count == h->capacity) {
    size_t const newcap = h->capacity * 2;
    h                   = realloc(h, sizeof *h + newcap * itemsize);
    if (h == NULL)
      return NULL;
    h->capacity = newcap;
    ptr         = h + 1;
  }
  if (depth)
    _deck_push_mark(h, depth - 1, h->count);
  h->count++;
  REQUIRE_WITH_MSG(h->count <= h->capacity, "Count cannot exceed capacity");
  return ptr;
}

void* _deck_push_impl(void* ptr, void* item, size_t const itemsize, uint8_t const depth)
{
  ptr            = _deck_push_empty(ptr, itemsize, depth);
  _DeckHeader* h = _deck_header(ptr);
  // Write into the last item we just pushed.
  memcpy((char*)ptr + (h->count - 1) * itemsize, item, itemsize);
  return ptr;
}

void* _deck_start_new_arr(void* ptr, size_t const itemsize, uint8_t const depth)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL) {
    size_t const bufsize = sizeof *h + itemsize;
    h                    = malloc(bufsize);
    if (h == NULL)
      return NULL;
    memset(h, 0, bufsize);
    h->capacity  = 1;
    h->item_size = itemsize;
    ptr          = (void*)(h + 1);
  }
  if (depth)
    _deck_push_mark(h, depth - 1, h->count);
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
    h->count     = 0;
    h->capacity  = n;
    h->item_size = itemsize;
    ptr          = h + 1;
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

static void _deck_calc_strides(_DeckHeader* h)
{
  if (h == NULL)
    return;
  arr_clear(h->pegs);
  arr_clear(h->stride_offset);
  arr_clear(h->strides);
  size_t const n_marks = arr_len(h->marks);
  {  // Exclusive scan of depth.
    size_t acc = 0;
    for (size_t i = 0; i < n_marks; ++i) {
      arr_push(h->stride_offset, acc);
      acc += (size_t)(h->marks[i].depth);
    }
  }
  {  // One stride entry per depth level per mark.
    size_t       n           = arr_len(h->stride_offset);
    size_t const last_offset = n > 0 ? h->stride_offset[n - 1] : 0;
    n                        = arr_len(h->marks);
    size_t const last_depth  = n > 0 ? h->marks[n - 1].depth : 0;
    size_t const n_total     = last_offset + last_depth;
    size_t const prevlen     = arr_len(h->strides);
    arr_resize(h->strides, n_total);
    memset(h->strides + prevlen, 0xff, (n_total - prevlen) * sizeof(*(h->strides)));
  }
  {  // Fill strides using pegs.
    for (size_t i = 0; i < n_marks; ++i) {
      size_t const d = h->marks[i].depth;
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
  OrcMark*     old_marks = h->marks;
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
      OrcMark const mark = {.depth = 0, .pos = j};
      arr_push(h->marks, mark);
    }
    OrcMark const mark = {.depth = new_depth, .pos = current};
    arr_push(h->marks, mark);
    prev = current + 1;
  }
  size_t const n_items = h->count;
  for (size_t j = prev; j < n_items; ++j) {
    OrcMark const mark = {.depth = 0, .pos = j};
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
    OrcMark* end = arr_end(h->marks);
    for (OrcMark* m = h->marks; m < end; ++m) {
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
    OrcMark* end = arr_end(h->marks);
    for (OrcMark* m = h->marks; m < end; ++m) {
      m->depth = remap[m->depth];
    }
  }
  _deck_calc_strides(h);
}

char* _deck_to_str(void*        ptr,
                   size_t const item_size,
                   void (*snprint_item)(void* item, char* dst, size_t len))
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL)
    return NULL;
  char*             output  = NULL;
  size_t const      n_marks = arr_len(h->marks);
  uint8_t const     dmax    = n_marks == 0 ? 0 : h->marks[0].depth;
  char const* const TAB     = "   ";
  Status            status  = OK;
  for (size_t mi = 0; mi < n_marks; ++mi) {
    size_t next_pos = (mi + 1) < n_marks ? h->marks[mi + 1].pos : h->count;
    if (next_pos > h->count) {
      next_pos = h->count;
    }
    {  // Left padding.
      uint8_t const n_left_pad = dmax - h->marks[mi].depth;
      for (uint8_t i = 0; i < n_left_pad; ++i) {
        status = str_push_str(output, TAB);
        REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      }
    }
    {  // Ruler marking - depth number.
      uint8_t const d_current    = h->marks[mi].depth + 1;
      char          depth_str[8] = {0};
      snprintf(depth_str, 8, "%3d ", d_current);
      status = str_push_str(output, depth_str);
      REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      // Ruler line.
      for (uint8_t i = 0; i < d_current; ++i) {
        status = str_push_str(output, "---");
        REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      }
      status = str_push(output, '|');
      REQUIRE_WITH_MSG(status == OK, "Allocation failed");
    }
    if (h->marks[mi].pos < next_pos) {  // Items
      // Write the first item without padding.
      size_t pos          = h->marks[mi].pos;
      char*  item         = (char*)ptr + pos * item_size;
      char   item_str[65] = {0};
      snprint_item(item, item_str, 64);
      item_str[64] = '\0';  // Just to be safe.
      status       = str_push(output, ' ');
      REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      status = str_push_str(output, item_str);
      REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      status = str_push(output, '\n');
      REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      // Write remaining items with padding.
      uint8_t const padding = dmax + 1;
      while (++pos < next_pos) {
        item += item_size;
        for (uint8_t i = 0; i < padding; ++i) {
          status = str_push_str(output, TAB);
          REQUIRE_WITH_MSG(status == OK, "Allocation failed");
        }
        status = str_push_str(output, "    | ");
        REQUIRE_WITH_MSG(status == OK, "Allocation failed");
        memset(item_str, 0, 65);
        snprint_item(item, item_str, 64);
        item_str[64] = '\0';  // Just to be safe.
        status       = str_push_str(output, item_str);
        REQUIRE_WITH_MSG(status == OK, "Allocation failed");
        status = str_push(output, '\n');
        REQUIRE_WITH_MSG(status == OK, "Allocation failed");
      }
    }
    else {
      status = str_push(output, '\n');
      REQUIRE_WITH_MSG(status == OK, "Allocation failed");
    }
  }
  return output;
}

// ========== DeckView ==========

static size_t _stride(OrcMark const*  marks,
                      uint64_t const* stride_offset,
                      size_t const    n_marks,
                      uint64_t const* strides,
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

DeckView _dv_from_deck_impl(void* ptr, size_t const item_size, uint8_t const depth)
{
  _DeckHeader* h = _deck_header(ptr);
  if (h == NULL)
    return (DeckView) {0};
  size_t const n_marks = arr_len(h->marks);
  return (DeckView) {
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

uint8_t dv_depth(DeckView const* const v)
{
  return v->depth;
}

size_t dv_len(DeckView const* const v)
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

DeckView dv_child(DeckView const* const v)
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
    return (DeckView) {.items         = v->items,
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
    return (DeckView) {
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

bool dv_advance(DeckView* const v)
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

void const* dv_item_ptr(DeckView const* const v)
{
  size_t const start_pos = v->depth == 0           ? v->start
                           : v->start < v->n_marks ? v->marks[v->start].pos
                                                   : v->n_items;
  return (char*)v->items + start_pos * v->item_size;
}

// ========== DeckWriter ==========

uint8_t _dw_next_depth(DeckWriter* writer)
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

DeckWriter dw_child(DeckWriter* writer)
{
  REQUIRE_WITH_MSG(writer != NULL && writer->depth > 0,
                   "Cannot create a child for this writer");
  uint8_t depth = writer->depth - 1;
  if (writer->has_next_depth) {
    depth                  = writer->next_depth;
    writer->has_next_depth = false;
  }
  return (DeckWriter) {
    .deck           = writer->deck,
    .item_size      = writer->item_size,
    .depth          = writer->depth - 1,
    .has_next_depth = true,
    .next_depth     = depth,
    .start          = deck_len(*(writer->deck)),
  };
}

Status _dw_push_impl(DeckWriter* writer, void* item)
{
  *(writer->deck) =
    _deck_push_impl(*(writer->deck), item, writer->item_size, _dw_next_depth(writer));
  return *(writer->deck) == NULL ? ALLOC_FAILED : OK;
}

Status dw_close(DeckWriter* writer)
{
  Status status = OK;
  if (writer != NULL && writer->has_next_depth) {
    *(writer->deck) =
      _deck_start_new_arr(*(writer->deck), writer->item_size, writer->next_depth);
    if (*(writer->deck) == NULL) {
      status = ALLOC_FAILED;
    }
    *writer = (DeckWriter) {0};
  }
  return status;
}

Status dw_advance(DeckWriter* writer)
{
  if (writer == NULL)
    return NULL_PTR;
  if (writer->has_next_depth) {
    *(writer->deck) =
      _deck_start_new_arr(*(writer->deck), writer->item_size, writer->next_depth);
    if (*(writer->deck) == NULL) {
      return ALLOC_FAILED;
    }
  }
  writer->has_next_depth = true;
  writer->next_depth     = writer->depth;
  writer->start          = deck_len(*(writer->deck));
  return OK;
}

void* dw_push_empty(DeckWriter* writer)
{
  *(writer->deck) =
    _deck_push_empty(*(writer->deck), writer->item_size, _dw_next_depth(writer));
  _DeckHeader* h = _deck_header(*(writer->deck));
  if (h == NULL) {
    return NULL;
  }
  void* ptr = (char*)(*(writer->deck)) + (h->count - 1) * writer->item_size;
  memset(ptr, 0, writer->item_size);
  return ptr;
}

// ========== Dims (Units) ==========

bool dims_equal(Dims const a, Dims const b)
{
  return memcmp(a, b, sizeof(*a) * ORC_NUM_DIMS) == 0;
}

void dims_multiply(Dims const a, Dims const b, Dims out)
{
  for (size_t i = 0; i < ORC_NUM_DIMS; ++i) {
    out[i] = a[i] + b[i];
  }
}

void dims_divide(Dims const a, Dims const b, Dims out)
{
  for (size_t i = 0; i < ORC_NUM_DIMS; ++i) {
    out[i] = a[i] - b[i];
  }
}

void dims_pow(Dims const a, int const pow, Dims out)
{
  for (size_t i = 0; i < ORC_NUM_DIMS; ++i) {
    out[i] = a[i] * pow;
  }
}

// ========== Combinations ==========

DeckView _dv_from_oh_impl(OrcHandle const* const handle, uint8_t const depth)
{
  if (handle == NULL)
    return (DeckView) {0};
  else
    return (DeckView) {
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
  DeckView*   view_matrix;
  uint8_t*    input_depths;
  size_t      n_inputs;
  DeckWriter* writer_matrix;
  uint8_t*    output_depths;
  size_t      n_outputs;
  size_t      stack_depth;
} Combinations;

void comb_free(void* ptr)
{
  if (ptr == NULL)
    return;
  Combinations* comb = (Combinations*)ptr;
  REQUIRE_WITH_MSG(comb->n_inputs == arr_len(comb->input_depths) &&
                     comb->n_outputs == arr_len(comb->output_depths),
                   "Invalid combinations");
  arr_free(comb->view_matrix);
  arr_free(comb->input_depths);
  arr_free(comb->writer_matrix);
  arr_free(comb->output_depths);
  free(comb);
}

uint8_t _oh_max_depth(OrcHandle const* handle)
{
  if (handle != NULL && handle->n_marks != 0) {
    return handle->marks[0].depth + 1;
  }
  return 0;
}

void oh_update(OrcHandle* handle)
{
  _DeckHeader* h        = _deck_header(handle->items);
  handle->handle        = (uint64_t)handle->items;
  handle->n_items       = h->count;
  handle->item_size     = h->item_size;
  handle->marks         = h->marks;
  handle->stride_offset = h->stride_offset;
  handle->n_marks       = arr_len(h->marks);
  handle->strides       = h->strides;
}

void* comb_init(OrcHandle const** inputs,
                uint8_t const*    input_depths,
                size_t const      n_inputs,
                OrcHandle**       outputs,
                uint8_t const*    output_depths,
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
  Combinations* out         = malloc(sizeof(Combinations));
  memset(out, 0, sizeof(Combinations));
  // Allocate buffers.
  arr_resize(out->view_matrix, n_inputs * stack_depth);
  arr_resize(out->writer_matrix, n_outputs * stack_depth);
  arr_resize(out->input_depths, n_inputs);
  arr_resize(out->output_depths, n_outputs);
  REQUIRE_WITH_MSG(out->view_matrix != NULL && out->writer_matrix != NULL &&
                     out->input_depths != NULL && out->output_depths != NULL,
                   "Allocation failed");
  // Populate input views and depths.
  for (size_t i = 0; i < n_inputs; ++i) {
    uint8_t const arg_depth = input_depths[i];
    out->input_depths[i]    = arg_depth;
    DeckView* dst           = out->view_matrix + i * stack_depth;
    // The first view.
    *dst = _dv_from_oh_impl(inputs[i], arg_depth + max_delta);
    // Telescope the views until we reach the target depth.
    for (size_t d = 1; d < stack_depth; ++d) {
      DeckView child = dv_child(dst);
      *(++dst)       = child;
    }
    REQUIRE(dst->depth == arg_depth);
  }
  // Populate output decks, writers, and depths.
  for (size_t i = 0; i < n_outputs; ++i) {
    uint8_t const arg_depth = output_depths[i];
    out->output_depths[i]   = arg_depth;
    DeckWriter* dst         = out->writer_matrix + i * stack_depth;
    // The first writer.
    *dst = (DeckWriter) {
      .deck           = &(outputs[i]->items),
      .item_size      = outputs[i]->item_size,
      .depth          = arg_depth + max_delta,
      .has_next_depth = true,
      .next_depth     = arg_depth + max_delta,
      .start          = deck_len(outputs[i]->items),
    };
    for (size_t d = 1; d < stack_depth; ++d) {
      DeckWriter child = dw_child(dst);
      *(++dst)         = child;
    }
    REQUIRE(dst->depth == arg_depth);
  }
  out->n_inputs    = n_inputs;
  out->n_outputs   = n_outputs;
  out->stack_depth = stack_depth;
  return out;
}

void* comb_advance(void* ptr)
{
  if (ptr == NULL)
    return NULL;
  Combinations* comb      = (Combinations*)ptr;
  size_t const  n_inputs  = comb->n_inputs;
  size_t const  n_outputs = comb->n_outputs;
  {  // Try to advance all the input views and check if at least one input advances.
    bool any_advanced = false;
    for (size_t i = 0; i < n_inputs; ++i) {
      DeckView*  last_view = comb->view_matrix + (i + 1) * comb->stack_depth - 1;
      bool const advanced  = dv_advance(last_view);
      any_advanced         = any_advanced || advanced;
    }
    if (any_advanced) {  // At least one input advanced. Advance all the writers.
      for (size_t i = 0; i < n_outputs; ++i) {
        DeckWriter*  last_writer = comb->writer_matrix + (i + 1) * comb->stack_depth - 1;
        Status const status      = dw_advance(last_writer);
        REQUIRE(status == OK);
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
  REQUIRE(comb->stack_depth > 0);
  size_t stack_top = comb->stack_depth - 1;
  do {
    if (stack_top == 0) {
      state = EXHAUSTED;
      break;
    }
    for (size_t i = 0; i < n_inputs; ++i) {  // Pop all the inputs.
      DeckView* last_view = comb->view_matrix + i * comb->stack_depth + stack_top;
      *last_view          = (DeckView) {0};
    }
    for (size_t i = 0; i < n_outputs; ++i) {  // Pop all the outputs.
      DeckWriter*  last_writer = comb->writer_matrix + i * comb->stack_depth + stack_top;
      Status const status      = dw_close(last_writer);
      REQUIRE(status == OK);
    }
    --stack_top;
    // Try to advance lower in the stack.
    for (size_t i = 0; i < n_inputs; ++i) {  // Advance all the inputs.
      DeckView* last_view = comb->view_matrix + i * comb->stack_depth + stack_top;
      if (dv_advance(last_view)) {
        state = ADVANCED;
      }
    }
    if (state == ADVANCED) {  // Only advance all the outputs if inputs did.
      for (size_t i = 0; i < n_outputs; ++i) {
        DeckWriter* last_writer = comb->writer_matrix + i * comb->stack_depth + stack_top;
        Status const status     = dw_advance(last_writer);
        REQUIRE(status == OK);
      }
    }
  } while (state == CONTINUE);
  switch (state) {
  case ADVANCED: {
    // Telescope the inputs to the desired depth.
    for (size_t i = 0; i < n_inputs; ++i) {
      DeckView* prev = comb->view_matrix + i * comb->stack_depth + stack_top;
      for (size_t d = stack_top + 1; d < comb->stack_depth; ++d) {
        DeckView* dst = prev + 1;
        *dst          = dv_child(prev);
        prev          = dst;
      }
    }
    // Telescope the outputs to the desired depth.
    for (size_t i = 0; i < n_outputs; ++i) {
      DeckWriter* prev = comb->writer_matrix + i * comb->stack_depth + stack_top;
      for (size_t d = stack_top + 1; d < comb->stack_depth; ++d) {
        DeckWriter* dst = prev + 1;
        *dst            = dw_child(prev);
        prev            = dst;
      }
    }
    return comb;
  } break;
  case EXHAUSTED: {
    comb_free(comb);
    return NULL;
  }
  case CONTINUE:  // fall through.
  default:
    REQUIRE_WITH_MSG(
      false,
      "Previous loop can only terminate when either we advance, or exhaust the inputs.");
    return NULL;
  }
}

DeckView comb_get_input(void* ptr, size_t const index)
{
  REQUIRE_WITH_MSG(ptr != NULL, "Invalid combinations");
  Combinations* comb = (Combinations*)ptr;
  REQUIRE_WITH_MSG(index < comb->n_inputs, "Index out of bounds");
  return *(comb->view_matrix + (index + 1) * comb->stack_depth - 1);
}

DeckWriter* comb_get_output(void* ptr, size_t const index)
{
  REQUIRE_WITH_MSG(ptr != NULL, "Invalid combinations");
  Combinations* comb = (Combinations*)ptr;
  REQUIRE_WITH_MSG(index < comb->n_outputs, "Index out of bounds");
  return comb->writer_matrix + (index + 1) * comb->stack_depth - 1;
}

// ========== FFI helper functions ==========

static OrcTypeInfo _orc_type_info_u8(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_U8, .opaque_id = 0},
                        .name    = "u8",
                        .desc    = "Unsigned 8 bit integer"};
}
static OrcTypeInfo _orc_type_info_u16(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_U16, .opaque_id = 0},
                        .name    = "u16",
                        .desc    = "Unsigned 16 bit integer"};
}
static OrcTypeInfo _orc_type_info_u32(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_U32, .opaque_id = 0},
                        .name    = "u32",
                        .desc    = "Unsigned 32 bit integer"};
}
static OrcTypeInfo _orc_type_info_u64(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_U64, .opaque_id = 0},
                        .name    = "u64",
                        .desc    = "Unsigned 64 bit integer"};
}
static OrcTypeInfo _orc_type_info_f32(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_F32, .opaque_id = 0},
                        .name    = "f32",
                        .desc    = "32 bit floating point scalar"};
}
static OrcTypeInfo _orc_type_info_f64(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_F64, .opaque_id = 0},
                        .name    = "f64",
                        .desc    = "64 bit floating point scalar"};
}
static OrcTypeInfo _orc_type_info_i8(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_I8, .opaque_id = 0},
                        .name    = "i8",
                        .desc    = "Signed 8 bit integer"};
}
static OrcTypeInfo _orc_type_info_i16(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_I16, .opaque_id = 0},
                        .name    = "i16",
                        .desc    = "Signed 16 bit integer"};
}
static OrcTypeInfo _orc_type_info_i32(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_I32, .opaque_id = 0},
                        .name    = "i32",
                        .desc    = "Signed 32 bit integer"};
}
static OrcTypeInfo _orc_type_info_i64(void)
{
  return (OrcTypeInfo) {.type_id = {.primitive_id = ORC_I64, .opaque_id = 0},
                        .name    = "i64",
                        .desc    = "Signed 64 bit integer"};
}

// ========== Plugin - Host IO ==========

void orc_plugin_init(OrcHost const* host, OrcPlugin* plugin_data_out)
{
  (void)host;
  (void)plugin_data_out;
  TODO("Not implemented");
}

void orc_plugin_data_free(OrcPlugin* plugin_data)
{
  (void)plugin_data;
  TODO("Not implemented");
}

// ========== Deck Allocations ==========

void orc_deck_alloc(OrcTypeId const id, OrcHandle* const out)
{
  if (out == NULL) {
    return;
  }
  out->item_size = 0;
  switch (id.primitive_id) {
    // Unsigned integers.
  case ORC_U8:
    out->item_size = sizeof(uint8_t);
    out->type_id   = _orc_type_info_u8().type_id;
    break;
  case ORC_U16:
    out->item_size = sizeof(uint16_t);
    out->type_id   = _orc_type_info_u16().type_id;
    break;
  case ORC_U32:
    out->item_size = sizeof(uint32_t);
    out->type_id   = _orc_type_info_u32().type_id;
    break;
  case ORC_U64:
    out->item_size = sizeof(uint64_t);
    out->type_id   = _orc_type_info_u64().type_id;
    break;
    // Scalars.
  case ORC_F32:
    out->item_size = sizeof(float);
    out->type_id   = _orc_type_info_f32().type_id;
    break;
  case ORC_F64:
    out->item_size = sizeof(double);
    out->type_id   = _orc_type_info_f64().type_id;
    break;
    // Signed integers.
  case ORC_I8:
    out->item_size = sizeof(int8_t);
    out->type_id   = _orc_type_info_i8().type_id;
    break;
  case ORC_I16:
    out->item_size = sizeof(int16_t);
    out->type_id   = _orc_type_info_i16().type_id;
    break;
  case ORC_I32:
    out->item_size = sizeof(int32_t);
    out->type_id   = _orc_type_info_i32().type_id;
    break;
  case ORC_I64:
    out->item_size = sizeof(int64_t);
    out->type_id   = _orc_type_info_i64().type_id;
    break;
  case ORC_OPAQUE:
    TODO("The plugin should handle its own types here");
  default:
    REQUIRE_WITH_MSG(false, "Unhandled type id. Should never happen.");
  }
  REQUIRE_WITH_MSG(out->item_size != 0, "Item size cannot be inferred from the type id.");
  size_t const INIT_SIZE = 1;
  void*        deck_ptr  = _deck_grow_capacity(NULL, out->item_size, INIT_SIZE);
  _DeckHeader* h         = _deck_header(deck_ptr);
  // Assign to the output deck.
  out->handle = (uint64_t)deck_ptr;
  out->items  = deck_ptr;
  REQUIRE_WITH_MSG(h->count == 0, "New deck must be empty");
  out->n_items         = h->count;
  out->marks           = h->marks;
  out->stride_offset   = h->stride_offset;
  size_t const n_marks = arr_len(h->marks);
  REQUIRE_WITH_MSG(n_marks == 0, "New deck must have no marks");
  out->n_marks = n_marks;
  out->strides = h->strides;
}

static void _free_deck_item(void* data, uint32_t const type_id)
{
  (void)data;
  (void)type_id;
  TODO(
    "This should be implemented for specific opaque types defined inside a plugin. For "
    "example, type_id could refer to a MESH type. Then the plugin casts the data pointer "
    "into a mesh pointer and frees it properly.");
}

void orc_deck_free(OrcHandle* const handle)
{
  REQUIRE_WITH_MSG(handle->handle == (uint64_t)handle->items,
                   "In this implementation the handle is just the pointer.");
  if (handle->type_id.primitive_id == ORC_OPAQUE) {
    // This is a custom type defined inside the plugin. The contents of the deck must be
    // freed first.
    size_t const count = deck_len(handle->items);
    char*        data  = (char*)handle->items;
    for (size_t i = 0; i < count; ++i, data += handle->item_size) {
      _free_deck_item(data, handle->type_id.opaque_id);
    }
  }
  _deck_free_impl(handle->items);  // Now we can free the deck container.
  memset(handle, 0, sizeof(OrcHandle));
}

static bool _orc_type_id_eq(OrcTypeId const a, OrcTypeId const b)
{
  return a.primitive_id == b.primitive_id && a.opaque_id == b.opaque_id;
}

void _copy_items_opaque(uint32_t     opaque_type_id,
                        void*        src,
                        void*        dst,
                        size_t const n_items)
{
  (void)opaque_type_id;
  (void)src;
  (void)dst;
  (void)n_items;
  TODO("The plugin has to implement the copy operation for it's type");
}

void orc_deck_from_proxy(OrcHandle const* inputs,
                         uint64_t const   n_inputs,
                         uint32_t const   proxy_type,
                         OrcHandle const* proxy,
                         OrcHandle*       out)
{
  if (n_inputs == 0) {
    return;  // Nothing to do.
  }
  REQUIRE_WITH_MSG(proxy->type_id.primitive_id == ORC_PROXY, "Invalid proxy deck");
  OrcTypeId const id        = inputs[0].type_id;
  size_t const    item_size = inputs[0].item_size;
  for (size_t i = 1; i < n_inputs; ++i) {
    REQUIRE_WITH_MSG(_orc_type_id_eq(id, inputs[i].type_id),
                     "All input decks must be of the same type");
  }
  orc_deck_alloc(id, out);
  REQUIRE_WITH_MSG(out->handle == (uint64_t)out->items,
                   "In this implementation the handle is just the pointer.");
  switch (proxy_type) {
  case ORC_DECK_PROXY_COPY_ALL: {
    REQUIRE_WITH_MSG(n_inputs == 1, "COPY_ALL is only valid with a single input.");
    size_t const n_items = inputs[0].n_items;
    void*        deck    = _deck_grow_capacity(out->items, item_size, n_items);
    _DeckHeader* h       = _deck_header(deck);
    h->item_size         = item_size;
    memcpy(out->dims, inputs[0].dims, sizeof(Dims));
    out->type_id = id;
    // Copy the data.
    if (id.primitive_id == ORC_OPAQUE) {
      memset(deck, 0, item_size * n_items);
      _copy_items_opaque(id.opaque_id, inputs[0].items, deck, n_items);
    }
    else {
      memcpy(deck, inputs[0].items, n_items * item_size);
    }
    h->count = n_items;
    // Copy the marks.
    size_t const n_marks = inputs[0].n_marks;
    h->marks             = _arr_resize_impl(h->marks, sizeof(OrcMark), n_marks);
    memcpy(h->marks, inputs[0].marks, sizeof(OrcMark) * n_marks);
    _deck_calc_strides(h);
    // Assign the pointer back to the handle and update the data inside the handle.
    out->items = deck;
    oh_update(out);
  } break;
  case ORC_DECK_PROXY_COPY_ITEMS: {
    REQUIRE_WITH_MSG(n_inputs == 1, "COPY_ITEMS is only valid with a single input.");
    size_t const n_items = inputs[0].n_items;
    void*        deck    = _deck_grow_capacity(out->items, item_size, n_items);
    _DeckHeader* h       = _deck_header(deck);
    h->item_size         = item_size;
    memcpy(out->dims, inputs[0].dims, sizeof(Dims));
    out->type_id = id;
    // Copy the data.
    if (id.primitive_id == ORC_OPAQUE) {
      memset(deck, 0, item_size * n_items);
      _copy_items_opaque(id.opaque_id, inputs[0].items, deck, n_items);
    }
    else {
      memcpy(deck, inputs[0].items, n_items * item_size);
    }
    h->count = n_items;
    // Copy the marks from the proxy, NOT the input.
    size_t const n_marks = proxy->n_marks;
    h->marks             = _arr_resize_impl(h->marks, sizeof(OrcMark), n_marks);
    memcpy(h->marks, proxy->marks, sizeof(OrcMark) * n_marks);
    _deck_calc_strides(h);
    // Update the handle pointer and update the rest of the data inside the handle.
    out->items = deck;
    oh_update(out);
  } break;
  case ORC_DECK_PROXY_SHUFFLE: {
    size_t const n_items = proxy->n_items;
    void*        deck    = _deck_grow_capacity(out->items, item_size, n_items);
    _DeckHeader* h       = _deck_header(deck);
    h->item_size         = item_size;
    memcpy(out->dims, proxy->dims, sizeof(Dims));
    out->type_id = id;
    // Copy the data one at a time from by iterating over the proxy.
    ItemProxy* proxies = (ItemProxy*)proxy->items;
    if (id.primitive_id == ORC_OPAQUE) {
      while (h->count < n_items) {
        REQUIRE_WITH_MSG(h->count < proxy->n_items && proxies[h->count].tree < n_inputs,
                         "Index out of bounds");
        void* src = (char*)inputs[proxies[h->count].tree].items +
                    item_size * proxies[h->count].item;
        void* dst = (char*)deck + item_size * h->count;
        _copy_items_opaque(id.opaque_id, src, dst, 1);
        ++h->count;
      }
    }
    else {
      while (h->count < n_items) {
        REQUIRE_WITH_MSG(h->count < proxy->n_items && proxies[h->count].tree < n_inputs,
                         "Index out of bounds");
        void* src = (char*)inputs[proxies[h->count].tree].items +
                    item_size * proxies[h->count].item;
        void* dst = (char*)deck + item_size * h->count;
        memcpy(dst, src, item_size);
        ++h->count;
      }
    }
    // Copy the marks from the proxy, NOT the input.
    size_t const n_marks = proxy->n_marks;
    h->marks             = _arr_resize_impl(h->marks, sizeof(OrcMark), n_marks);
    memcpy(h->marks, proxy->marks, sizeof(OrcMark) * n_marks);
    _deck_calc_strides(h);
    // Update the handle pointer and update the rest of the data inside the handle.
    out->items = deck;
    oh_update(out);
  } break;
  default:
    REQUIRE_WITH_MSG(false, "Invalid proxy type");
    break;
  }
}
