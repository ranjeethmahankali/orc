#pragma once

#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "orc_ffi.h"

#define ORC_SDK_REQUIRE_WITH_MSG(cond, msg)                                            \
  do {                                                                                 \
    if (!(cond)) {                                                                     \
      fprintf(stderr, "\n\n%s:%d: REQUIRE FAILED: (%s)\n", __FILE__, __LINE__, #cond); \
      if (msg != NULL) {                                                               \
        fprintf(stderr, "  %s", (char *)msg);                                          \
        fflush(stderr);                                                                \
      }                                                                                \
      fprintf(stderr, "\n");                                                           \
      fflush(stderr);                                                                  \
      abort();                                                                         \
    }                                                                                  \
  } while (0)

#define ORC_SDK_REQUIRE(cond) ORC_SDK_REQUIRE_WITH_MSG(cond, NULL)

#define ORC_SDK_TODO(msg)                               \
  do {                                                  \
    fprintf(stderr, "TODO: %s:%d", __FILE__, __LINE__); \
    if (msg != NULL) {                                  \
      fprintf(stderr, "  %s", (char *)msg);             \
    }                                                   \
    fprintf(stderr, "\n");                              \
    fflush(stderr);                                     \
    abort();                                            \
  } while (0)

#define ORC_SDK_DEBUG(...) (fprintf(stderr, __VA_ARGS__))

typedef enum
{
  OK = 0,
  ALLOC_FAILED,
  OUT_OF_BOUNDS,
  NULL_PTR,
} OrcSdk_Status;

// ========== Array ==========

typedef struct
{
  size_t count;
  size_t capacity;
} _OrcSdk_ArrHeader;

static inline _OrcSdk_ArrHeader *_orc_sdk_arr_header(void const *ptr)
{
  _OrcSdk_ArrHeader *h = (_OrcSdk_ArrHeader *)ptr;
  if (h)
    return --h;
  return NULL;
}

static inline size_t _orc_sdk_arr_capacity(void *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h)
    return h->capacity;
  return 0;
}

/**
 * Free the array and assign the pointer to NULL.
 */
#define orc_sdk_arr_free(ptr) (free(_orc_sdk_arr_header((ptr))), (ptr) = NULL)

/**
 * @brief Get the length of the array.
 *
 * @param ptr Pointer to the array.
 * @return size_t Length.
 */
static inline size_t orc_sdk_arr_len(void *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h)
    return h->count;
  return 0;
}

void *_orc_sdk_arr_grow(void *ptr, size_t elemsize);

OrcSdk_Status _orc_sdk_arr_remove_impl(void        *ptr,
                                       size_t const idx,
                                       size_t const elemsize);

/**
 * Remove the element at index idx from the array. This preserves the order of the
 * remaining elements.
 */
#define orc_sdk_arr_remove(ptr, idx) _orc_sdk_arr_remove_impl((ptr), (idx), sizeof *(ptr))

/**
 * Push a new value to the end of the array.
 */
#define orc_sdk_arr_push(ptr, val)                              \
  (((ptr) = _orc_sdk_arr_grow(ptr, sizeof *(ptr)))              \
     ? ((ptr)[_orc_sdk_arr_header((ptr))->count++] = (val), OK) \
     : ALLOC_FAILED)

/**
 * Get a pointer pointing past the end of the array.
 */
#define orc_sdk_arr_end(ptr) (ptr) + orc_sdk_arr_len((ptr))

/**
 * Removes the element at the index idx from the array. This is faster than
 * orc_sdk_arr_remove, but doesn't preserve the order of the elements.
 */
#define orc_sdk_arr_swap_remove(ptr, idx)                                \
  (((ptr) != NULL && (idx) < orc_sdk_arr_len(ptr))                       \
     ? ((ptr)[(idx)] = (ptr)[--(_orc_sdk_arr_header((ptr))->count)], OK) \
     : OUT_OF_BOUNDS)

void *_orc_sdk_arr_grow_capacity(void *ptr, size_t const elemsize, size_t const nelems);

/**
 * Reserve memory for the `size` number of elements in the array.
 */
#define orc_sdk_arr_reserve(ptr, size)                                \
  (((ptr) = _orc_sdk_arr_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? OK                                                             \
     : ((size) == 0 ? OK : ALLOC_FAILED))

void *_orc_sdk_arr_resize_impl(void *ptr, size_t const elemsize, size_t const count);

/**
 * Resize the array to the given size. If the new size is smaller than the original size,
 * the array is simply truncated. If the size is longer than the original size, the array
 * is extended, and the newly allocated space is zero'd out. Existing elements remain as
 * they were.
 */
#define orc_sdk_arr_resize(ptr, size) \
  (ptr) = _orc_sdk_arr_resize_impl((ptr), sizeof *(ptr), (size))

/**
 * @brief Clear all elements from the array. This doesn't de-allocate the memory.
 *
 * @param ptr Pointer to the array.
 */
void orc_sdk_arr_clear(void *ptr);

void _orc_sdk_arr_fill_impl(void             *arr,
                            void const *const elem,
                            size_t const      count,
                            size_t const      elemsize);

#define orc_sdk_arr_fill(ptr, val) \
  _orc_sdk_arr_fill_impl((ptr), &(val), orc_sdk_arr_len((ptr)), sizeof(*ptr))

OrcSdk_Status _orc_sdk_arr_remove_range_impl(void        *ptr,
                                             size_t const start,
                                             size_t const stop,
                                             size_t const elemsize);

/**
 * Remove a range of elements from the array. The range is from `start` (inclusive) to
 * `stop` (exclusive).
 */
#define orc_sdk_arr_remove_range(ptr, start, stop) \
  _orc_sdk_arr_remove_range_impl(ptr, start, stop, sizeof *(ptr))

/**
 * Remove the last element of the array and assign it to the `dst` pointer. OK status is
 * returned if the array was not empty. OUT_OF_BOUNDS status is returned if the array was
 * empty.
 */
#define orc_sdk_arr_pop(ptr, dst)                                \
  (((ptr) != NULL && orc_sdk_arr_len((ptr)) > 0)                 \
     ? (*dst = (ptr)[--(_orc_sdk_arr_header((ptr)))->count], OK) \
     : OUT_OF_BOUNDS)

/**
 * @brief Check if the array is empty.
 *
 * @param ptr Pointer to the array.
 * @return bool True if the array is empty.
 */
bool orc_sdk_arr_is_empty(void *ptr);

// ========== String ==========

#define orc_str_free(ptr) orc_sdk_arr_free((ptr))

static inline size_t orc_str_len(void *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h)
    return h->count == 0 ? 0 : h->count - 1;
  return 0;
}

OrcSdk_Status _orc_str_remove_impl(char *const ptr, size_t const idx);

/**
 * @brief Remove a character from a position in a stirng.
 *
 * @param ptr Pointer to the string. This will be overwritten by the macro.
 * @param idx Index to remove the character at.
 */
#define orc_str_remove(ptr, idx) _orc_str_remove_impl((ptr), (idx))

char *_orc_str_push_impl(char *, char);

/**
 * @brief Push a character to the end of the string.
 *
 * @param ptr Pointer to the string. This maybe overwriiten by the macro if an allocation
 * happens.
 * @param ch Character to push.
 */
#define orc_str_push(ptr, ch) \
  (((ptr) = _orc_str_push_impl((ptr), (ch))) ? OK : ALLOC_FAILED)

/**
 * @brief Get the pointer past the end of the string.
 *
 * @param ptr Pointer to the string.
 * @return Pointer just past the last character of the string. Points to the null
 * terminator.
 */
#define orc_str_end(ptr) ((ptr) + orc_str_len((ptr)))

/**
 * @brief Clear the contents of a string.
 *
 * @param ptr The string to clear.
 */
void orc_str_clear(char *ptr);

char *_orc_str_push_str_impl(char *, char const *);

/**
 * @brief Push the `tail` string at the end of the `ptr` string.
 *
 * @param dst The string to append to.
 * @param tail The string to append.
 */
#define orc_str_push_str(dst, tail) \
  (((dst) = _orc_str_push_str_impl((dst), (tail))) ? OK : ALLOC_FAILED)

/**
 * @brief Check if the string is empty.
 *
 * @param ptr The string.
 * @return bool Flag indicating if the string is empty.
 */
static inline bool orc_str_is_empty(void *ptr)
{
  return orc_str_len(ptr) == 0;
}

bool orc_str_eq(char *const a, char *const b);

// ========== String views ==========

typedef struct
{
  char *start;
  char *end;
} OrcStrView;

#define orc_sv_len(sv) \
  (((sv).start == NULL || (sv).end == NULL) ? 0 : (size_t)((sv).end - (sv).start))

static inline bool orc_sv_is_empty(OrcStrView const sv)
{
  return sv.start == sv.end || sv.start == NULL || sv.end == NULL;
}

OrcStrView orc_sv_from_str(char *str);

OrcStrView orc_sv_trim_left(OrcStrView sv);

OrcStrView orc_sv_trim_right(OrcStrView sv);

static inline OrcStrView orc_sv_trim(OrcStrView sv)
{
  return orc_sv_trim_left(orc_sv_trim_right(sv));
}

bool orc_sv_starts_with(OrcStrView const sv, char const *const prefix);

bool orc_sv_ends_with(OrcStrView const sv, char const *const suffix);

bool orc_sv_strip_prefix(OrcStrView *sv, char const *const prefix);

bool orc_sv_strip_suffix(OrcStrView *sv, char const *const suffix);

bool orc_sv_contains_str(OrcStrView const sv, char const *const needle);

OrcStrView orc_sv_split_at_delim(OrcStrView *sv, char const delim);

#define orc_sv_split_line(sv) orc_sv_split_at_delim((sv), '\n')

char *orc_sv_find(OrcStrView sv, char const c);

static inline bool orc_sv_contains_char(OrcStrView sv, char const c)
{
  return orc_sv_find(sv, c) != NULL;
}

char *orc_sv_rfind(OrcStrView sv, char const c);

OrcStrView orc_sv_slice(OrcStrView sv, size_t const start, size_t const end);

bool orc_sv_eq(OrcStrView const a, OrcStrView const b);

// ========== Deck ==========

typedef struct
{
  OrcMark  *marks;
  uint64_t *stride_offset;
  uint64_t *strides;
  size_t   *pegs;
  size_t    count;
  size_t    capacity;
  size_t    item_size;
  size_t    _alignment;
} _OrcSdk_DeckHeader;

static inline _OrcSdk_DeckHeader *_orc_sdk_deck_header(void const *ptr)
{
  _OrcSdk_DeckHeader *h = (_OrcSdk_DeckHeader *)ptr;
  if (h)
    return --h;
  return NULL;
}

void _orc_sdk_deck_free_impl(void *ptr);

#define orc_sdk_deck_free(ptr) (_orc_sdk_deck_free_impl((ptr)), (ptr) = NULL)

uint8_t orc_sdk_deck_max_depth(void const *deck);

static inline size_t orc_sdk_deck_len(void const *deck)
{
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  if (h)
    return h->count;
  return 0;
}

static inline bool orc_sdk_deck_is_empty(void *deck)
{
  return orc_sdk_deck_len(deck) == 0;
}

void *_orc_sdk_deck_push_impl(void *ptr, void *item, size_t const itemsize, uint8_t const depth);

#define orc_sdk_deck_push(ptr, item, depth) \
  (((ptr) = _orc_sdk_deck_push_impl((ptr), &(item), sizeof(*ptr), (depth))) ? OK : ALLOC_FAILED)

void *_orc_sdk_deck_start_new_arr(void *ptr, size_t const itemsize, uint8_t const depth);

#define orc_sdk_deck_start_arr(ptr, depth) \
  (((ptr) = _orc_sdk_deck_start_new_arr((ptr), sizeof(*(ptr)), (depth))) ? OK : ALLOC_FAILED)

void orc_sdk_deck_clear(void const *ptr);

void *_orc_sdk_deck_grow_capacity(void *ptr, size_t const itemsize, size_t const n);

#define orc_sdk_deck_reserve(ptr, size)                                \
  (((ptr) = _orc_sdk_deck_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? OK                                                      \
     : ((size) == 0 ? OK : ALLOC_FAILED))

void orc_sdk_deck_flatten(void *ptr);

void orc_sdk_deck_graft(void *ptr);

void orc_sdk_deck_simplify(void *ptr);

char *_orc_sdk_deck_to_str(void        *ptr,
                   size_t const item_size,
                   void (*snprint_item)(void *item, char *dst, size_t len));

#define orc_sdk_deck_to_str(ptr, snprint_item) _orc_sdk_deck_to_str((ptr), sizeof(*(ptr)), snprint_item)

/* ========== ORC_SDK_DECK_INIT macro ==========
 *
 * Initialise a deck from a nested parenthesised literal, mirroring the Rust
 * `deck!` macro.  Depths are assigned with the "ruler-sequence" rule: the first
 * group at each nesting level inherits the assigned depth; subsequent siblings
 * compute their own intrinsic depth.
 *
 * Usage:
 *   size_t* d = NULL;
 *   ORC_SDK_DECK_INIT(d, size_t, (1, 2, 3));                            // depth 1
 *   ORC_SDK_DECK_INIT(d, size_t, ((1, 2, 3), (4, 5, 6), (7, 8, 9)));   // depth 2
 *   ORC_SDK_DECK_INIT(d, size_t, (((1, 2), (3, 4)), ((5, 6), (7, 8)))); // depth 3
 *   orc_sdk_deck_free(d);
 *
 * Supports nesting up to depth 255 (limited by rescan budget: 3^6 = 729).
 */
/* --- Eval chain: forces 3^6 = 729 rescans --- */
#define _ORC_SDK_DI_EVAL(...) _ORC_SDK_DI_E1(_ORC_SDK_DI_E1(_ORC_SDK_DI_E1(__VA_ARGS__)))
#define _ORC_SDK_DI_E1(...) _ORC_SDK_DI_E2(_ORC_SDK_DI_E2(_ORC_SDK_DI_E2(__VA_ARGS__)))
#define _ORC_SDK_DI_E2(...) _ORC_SDK_DI_E3(_ORC_SDK_DI_E3(_ORC_SDK_DI_E3(__VA_ARGS__)))
#define _ORC_SDK_DI_E3(...) _ORC_SDK_DI_E4(_ORC_SDK_DI_E4(_ORC_SDK_DI_E4(__VA_ARGS__)))
#define _ORC_SDK_DI_E4(...) _ORC_SDK_DI_E5(_ORC_SDK_DI_E5(_ORC_SDK_DI_E5(__VA_ARGS__)))
#define _ORC_SDK_DI_E5(...) __VA_ARGS__

/* --- Core utility --- */
#define _ORC_SDK_DI_EMPTY()
#define _ORC_SDK_DI_DEFER(id) id _ORC_SDK_DI_EMPTY()
#define _ORC_SDK_DI_EXPAND(...) __VA_ARGS__
#define _ORC_SDK_DI_CAT(a, b) _ORC_SDK_DI_CAT_(a, b)
#define _ORC_SDK_DI_CAT_(a, b) a##b
#define _ORC_SDK_DI_FIRST(...) _ORC_SDK_DI_FIRST_(__VA_ARGS__, ~)
#define _ORC_SDK_DI_FIRST_(a, ...) a
#define _ORC_SDK_DI_SECOND(...) _ORC_SDK_DI_SECOND_(__VA_ARGS__, ~, ~)
#define _ORC_SDK_DI_SECOND_(a, b, ...) b

/* --- Paren detection: _ORC_SDK_DI_IS_PAREN(x) => 1 if x is (...), else 0 --- */
#define _ORC_SDK_DI_IS_PAREN_PROBE(...) ~, 1
#define _ORC_SDK_DI_IS_PAREN(x) _ORC_SDK_DI_IS_PAREN_CHK(_ORC_SDK_DI_IS_PAREN_PROBE x)
#define _ORC_SDK_DI_IS_PAREN_CHK(...) _ORC_SDK_DI_SECOND(__VA_ARGS__, 0)

/* --- Boolean --- */
#define _ORC_SDK_DI_NOT(x) _ORC_SDK_DI_CAT(_ORC_SDK_DI_NOT_, x)
#define _ORC_SDK_DI_NOT_0 1
#define _ORC_SDK_DI_NOT_1 0

/* --- End sentinel (function-like macro; detected via invocation, not ##).
 *     _ORC_SDK_DI_DONE_CHK is a probe: for empty x it sees () directly; for
 * _ORC_SDK_DI_END the sentinel eats () instead.  Either way ~,1 is injected.  For normal
 *     values (including 1.1) neither fires, so _ORC_SDK_DI_SECOND returns 0. --- */
#define _ORC_SDK_DI_END(...) ~, 1
#define _ORC_SDK_DI_DONE_CHK() ~, 1
#define _ORC_SDK_DI_IS_END(x) _ORC_SDK_DI_SECOND(_ORC_SDK_DI_DONE_CHK x(), 0)
#define _ORC_SDK_DI_CONT(x) _ORC_SDK_DI_CAT(_ORC_SDK_DI_CONT_, _ORC_SDK_DI_IS_PAREN(x))(x)
#define _ORC_SDK_DI_CONT_1(x) 1
#define _ORC_SDK_DI_CONT_0(x) _ORC_SDK_DI_NOT(_ORC_SDK_DI_IS_END(x))

/* --- Push: main dispatch (CAT-based to avoid IIF prescanning) ---
 *     Depth is threaded as an accumulator: +1 at each paren-strip.
 *     Rest-groups always restart at depth 1 (intrinsic depth). */
#define _ORC_SDK_DI_PUSH(ptr, type, depth, ...)                                      \
  _ORC_SDK_DI_CAT(_ORC_SDK_DI_PC_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__))) \
  (ptr, type, depth, __VA_ARGS__)
#define _ORC_SDK_DI_PC_0(ptr, type, depth, ...) \
  (void)((ptr) = _orc_sdk_deck_start_new_arr((void *)(ptr), sizeof(type), (uint8_t)(depth)));
#define _ORC_SDK_DI_PC_1(ptr, type, depth, ...)                                          \
  _ORC_SDK_DI_CAT(_ORC_SDK_DI_PP_, _ORC_SDK_DI_IS_PAREN(_ORC_SDK_DI_FIRST(__VA_ARGS__))) \
  (ptr, type, depth, __VA_ARGS__)
#define _ORC_SDK_DI_PP_0(ptr, type, depth, ...) \
  _ORC_SDK_DI_PUSH_VALS(ptr, type, depth, __VA_ARGS__)
#define _ORC_SDK_DI_PP_1(ptr, type, depth, first_grp, ...)                              \
  _ORC_SDK_DI_DEFER(_ORC_SDK_DI_PUSH_I)                                                 \
  ()(ptr, type, depth + 1, _ORC_SDK_DI_EXPAND first_grp, _ORC_SDK_DI_END)               \
    _ORC_SDK_DI_CAT(_ORC_SDK_DI_GT_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__)))( \
      ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_PUSH_I() _ORC_SDK_DI_PUSH

/* --- Push flat values: first gets depth, rest get 0 --- */
#define _ORC_SDK_DI_PUSH_VALS(ptr, type, depth, first, ...)                          \
  (void)((ptr) = _orc_sdk_deck_push_impl(                                                    \
           (void *)(ptr), &((type) {first}), sizeof(type), (uint8_t)(depth)));       \
  _ORC_SDK_DI_CAT(_ORC_SDK_DI_VT_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__))) \
  (ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_PUSH_VALS_REST(ptr, type, first, ...)                             \
  (void)((ptr) = _orc_sdk_deck_push_impl((void *)(ptr), &((type) {first}), sizeof(type), 0)); \
  _ORC_SDK_DI_CAT(_ORC_SDK_DI_VT_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__)))  \
  (ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_VT_0(ptr, type, ...)
#define _ORC_SDK_DI_VT_1(ptr, type, ...) \
  _ORC_SDK_DI_DEFER(_ORC_SDK_DI_PUSH_VALS_REST_I)()(ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_PUSH_VALS_REST_I() _ORC_SDK_DI_PUSH_VALS_REST

/* --- Rest groups: each restarts at depth 1 --- */
#define _ORC_SDK_DI_GT_0(ptr, type, ...)
#define _ORC_SDK_DI_GT_1(ptr, type, ...) \
  _ORC_SDK_DI_DEFER(_ORC_SDK_DI_PUSH_REST_GRP_I)()(ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_PUSH_REST_GRP(ptr, type, grp, ...)                                  \
  _ORC_SDK_DI_DEFER(_ORC_SDK_DI_PUSH_I)                                                 \
  ()(ptr, type, 1, _ORC_SDK_DI_EXPAND grp, _ORC_SDK_DI_END)                             \
    _ORC_SDK_DI_CAT(_ORC_SDK_DI_GT_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__)))( \
      ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_PUSH_REST_GRP_I() _ORC_SDK_DI_PUSH_REST_GRP

/* --- Unwrap: strip outer parens if present, pass through bare values --- */
#define _ORC_SDK_DI_UNWRAP(x) _ORC_SDK_DI_CAT(_ORC_SDK_DI_UW_, _ORC_SDK_DI_IS_PAREN(x))(x)
#define _ORC_SDK_DI_UW_1(x) _ORC_SDK_DI_EXPAND x
#define _ORC_SDK_DI_UW_0(x) x

/* --- Entry point --- */
#define ORC_SDK_DECK_INIT(ptr, type, data)                                         \
  do {                                                                             \
    orc_sdk_deck_clear((void *)(ptr));                                                     \
    _ORC_SDK_DI_EVAL(                                                              \
      _ORC_SDK_DI_PUSH((ptr), type, 1, _ORC_SDK_DI_UNWRAP(data), _ORC_SDK_DI_END)) \
  } while (0)

// ========== DeckView ==========

typedef struct
{
  void const     *items;
  size_t          n_items;
  size_t          item_size;
  OrcMark const  *marks;
  uint64_t const *stride_offset;
  size_t          n_marks;
  uint64_t const *strides;
  uint8_t         depth;
  size_t          start;
  size_t          end;
} OrcSdk_DeckView;

OrcSdk_DeckView _orc_sdk_dv_from_deck_impl(void *ptr, size_t const item_size, uint8_t const depth);

#define dv_from_deck(ptr, depth) _orc_sdk_dv_from_deck_impl((ptr), sizeof(*(ptr)), (depth))

uint8_t dv_depth(OrcSdk_DeckView const *const v);

size_t dv_len(OrcSdk_DeckView const *const v);

OrcSdk_DeckView dv_child(OrcSdk_DeckView const *const v);

void const *dv_item_ptr(OrcSdk_DeckView const *const v);

bool dv_advance(OrcSdk_DeckView *const v);

// ========== DeckWriter ==========

typedef struct
{
  void  **deck;
  size_t  item_size;
  uint8_t depth;
  bool    has_next_depth;
  uint8_t next_depth;
  size_t  start;
} DeckWriter;

static inline DeckWriter _dw_from_deck_impl(void        **deck,
                                            uint8_t const depth,
                                            size_t const  item_size)
{
  return (DeckWriter) {
    .deck           = deck,
    .item_size      = item_size,
    .depth          = depth,
    .has_next_depth = true,
    .next_depth     = depth,
    .start          = orc_sdk_deck_len(*deck),
  };
}

#define dw_from_deck(deck, depth) \
  (_dw_from_deck_impl((void **)(&(deck)), (depth), sizeof(*(deck))))

DeckWriter dw_child(DeckWriter *writer);

uint8_t _dw_next_depth(DeckWriter *writer);

OrcSdk_Status _dw_push_impl(DeckWriter *writer, void *item);

#define dw_push(writer, item) (_dw_push_impl((writer), &(item)))

void *dw_push_empty(DeckWriter *writer);

static inline void *deck_item_ptr(DeckWriter *writer)
{
  return (char *)(*(writer->deck)) + writer->start * writer->item_size;
}

static inline size_t dw_len(DeckWriter *writer)
{
  return orc_sdk_deck_len(*(writer->deck)) - writer->start;
}

OrcSdk_Status dw_close(DeckWriter *writer);

OrcSdk_Status dw_advance(DeckWriter *writer);

// ========== Dims (Units) ==========

bool dims_equal(OrcDims const a, OrcDims const b);

void dims_multiply(OrcDims const a, OrcDims const b, OrcDims out);

void dims_divide(OrcDims const a, OrcDims const b, OrcDims out);

void dims_pow(OrcDims const a, int const pow, OrcDims out);

// ========== Combinations ==========

void *comb_init(OrcHandle const **inputs,
                uint8_t const    *input_depths,
                size_t const      n_inputs,
                OrcHandle       **outputs,
                uint8_t const    *output_depths,
                size_t const      n_outputs);

void comb_free(void *comb);

void *comb_advance(void *comb);

OrcSdk_DeckView comb_get_input(void *comb, size_t const index);

DeckWriter *comb_get_output(void *comb, size_t const index);

void oh_update(OrcHandle *handle);

// ========== Helpers for implementing plugins ==========

OrcError orc_sdk_handle_alloc(OrcTypeId const id, OrcHandle *const out);

OrcError orc_sdk_handle_free(OrcHandle *const handle);

OrcError orc_sdk_deck_from_proxy(OrcHandle const   *inputs,
                                 uint64_t const     n_inputs,
                                 OrcProxyType const proxy_type,
                                 OrcHandle const   *proxy,
                                 OrcHandle         *out);
