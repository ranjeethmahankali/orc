#pragma once

#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "orc_ffi.h"

#define REQUIRE_WITH_MSG(cond, msg)                                                    \
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

#define REQUIRE(cond) REQUIRE_WITH_MSG(cond, NULL)

#define TODO(msg)                                       \
  do {                                                  \
    fprintf(stderr, "TODO: %s:%d", __FILE__, __LINE__); \
    if (msg != NULL) {                                  \
      fprintf(stderr, "  %s", (char *)msg);             \
    }                                                   \
    fprintf(stderr, "\n");                              \
    fflush(stderr);                                     \
    abort();                                            \
  } while (0)

#define DEBUG(...) (fprintf(stderr, __VA_ARGS__))

// The purpose of this struct is to check for maximum alignment compatibility of other
// types.
typedef union
{
  long long   ll;
  long double ld;
  void       *p;
} _MaxAlignCompat;

typedef enum
{
  OK = 0,
  ALLOC_FAILED,
  OUT_OF_BOUNDS,
  NULL_PTR,
} Status;

// ========== Array ==========

typedef struct
{
  size_t count;
  size_t capacity;
} _ArrHeader;

static inline _ArrHeader *_arr_header(void const *ptr)
{
  _ArrHeader *h = (_ArrHeader *)ptr;
  if (h)
    return --h;
  return NULL;
}

static inline size_t _arr_capacity(void *ptr)
{
  _ArrHeader *h = _arr_header(ptr);
  if (h)
    return h->capacity;
  return 0;
}

/**
 * Free the array and assign the pointer to NULL.
 */
#define arr_free(ptr) (free(_arr_header((ptr))), (ptr) = NULL)

/**
 * @brief Get the length of the array.
 *
 * @param ptr Pointer to the array.
 * @return size_t Length.
 */
static inline size_t arr_len(void *ptr)
{
  _ArrHeader *h = _arr_header(ptr);
  if (h)
    return h->count;
  return 0;
}

void *_arr_grow(void *ptr, size_t elemsize);

Status _arr_remove_impl(void *ptr, size_t const idx, size_t const elemsize);

/**
 * Remove the element at index idx from the array. This preserves the order of the
 * remaining elements.
 */
#define arr_remove(ptr, idx) _arr_remove_impl((ptr), (idx), sizeof *(ptr))

/**
 * Push a new value to the end of the array.
 */
#define arr_push(ptr, val)                              \
  (((ptr) = _arr_grow(ptr, sizeof *(ptr)))              \
     ? ((ptr)[_arr_header((ptr))->count++] = (val), OK) \
     : ALLOC_FAILED)

/**
 * Get a pointer pointing past the end of the array.
 */
#define arr_end(ptr) (ptr) + arr_len((ptr))

/**
 * Removes the element at the index idx from the array. This is faster than arr_remove,
 * but doesn't preserve the order of the elements.
 */
#define arr_swap_remove(ptr, idx)                                \
  (((ptr) != NULL && (idx) < arr_len(ptr))                       \
     ? ((ptr)[(idx)] = (ptr)[--(_arr_header((ptr))->count)], OK) \
     : OUT_OF_BOUNDS)

void *_arr_grow_capacity(void *ptr, size_t const elemsize, size_t const nelems);

/**
 * Reserve memory for the `size` number of elements in the array.
 */
#define arr_reserve(ptr, size)                                \
  (((ptr) = _arr_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? OK                                                     \
     : ((size) == 0 ? OK : ALLOC_FAILED))

void *_arr_resize_impl(void *ptr, size_t const elemsize, size_t const count);

/**
 * Resize the array to the given size. If the new size is smaller than the original size,
 * the array is simply truncated. If the size is longer than the original size, the array
 * is extended, and the newly allocated space is zero'd out. Existing elements remain as
 * they were.
 */
#define arr_resize(ptr, size) (ptr) = _arr_resize_impl((ptr), sizeof *(ptr), (size))

/**
 * @brief Clear all elements from the array. This doesn't de-allocate the memory.
 *
 * @param ptr Pointer to the array.
 */
void arr_clear(void *ptr);

void _arr_fill_impl(void             *arr,
                    void const *const elem,
                    size_t const      count,
                    size_t const      elemsize);

#define arr_fill(ptr, val) _arr_fill_impl((ptr), &(val), arr_len((ptr)), sizeof(*ptr))

Status _arr_remove_range_impl(void        *ptr,
                              size_t const start,
                              size_t const stop,
                              size_t const elemsize);

/**
 * Remove a range of elements from the array. The range is from `start` (inclusive) to
 * `stop` (exclusive).
 */
#define arr_remove_range(ptr, start, stop) \
  _arr_remove_range_impl(ptr, start, stop, sizeof *(ptr))

/**
 * Remove the last element of the array and assign it to the `dst` pointer. OK status is
 * returned if the array was not empty. OUT_OF_BOUNDS status is returned if the array was
 * empty.
 */
#define arr_pop(ptr, dst)                                \
  (((ptr) != NULL && arr_len((ptr)) > 0)                 \
     ? (*dst = (ptr)[--(_arr_header((ptr)))->count], OK) \
     : OUT_OF_BOUNDS)

/**
 * @brief Check if the array is empty.
 *
 * @param ptr Pointer to the array.
 * @return bool True if the array is empty.
 */
bool arr_is_empty(void *ptr);

// ========== String ==========

#define str_free(ptr) arr_free((ptr))

static inline size_t str_len(void *ptr)
{
  _ArrHeader *h = _arr_header(ptr);
  if (h)
    return h->count == 0 ? 0 : h->count - 1;
  return 0;
}

Status _str_remove_impl(char *const ptr, size_t const idx);

/**
 * @brief Remove a character from a position in a stirng.
 *
 * @param ptr Pointer to the string. This will be overwritten by the macro.
 * @param idx Index to remove the character at.
 */
#define str_remove(ptr, idx) _str_remove_impl((ptr), (idx))

char *_str_push_impl(char *, char);

/**
 * @brief Push a character to the end of the string.
 *
 * @param ptr Pointer to the string. This maybe overwriiten by the macro if an allocation
 * happens.
 * @param ch Character to push.
 */
#define str_push(ptr, ch) (((ptr) = _str_push_impl((ptr), (ch))) ? OK : ALLOC_FAILED)

/**
 * @brief Get the pointer past the end of the string.
 *
 * @param ptr Pointer to the string.
 * @return Pointer just past the last character of the string. Points to the null
 * terminator.
 */
#define str_end(ptr) ((ptr) + str_len((ptr)))

/**
 * @brief Clear the contents of a string.
 *
 * @param ptr The string to clear.
 */
void str_clear(char *ptr);

char *_str_push_str_impl(char *, char const *);

/**
 * @brief Push the `tail` string at the end of the `ptr` string.
 *
 * @param dst The string to append to.
 * @param tail The string to append.
 */
#define str_push_str(dst, tail) \
  (((dst) = _str_push_str_impl((dst), (tail))) ? OK : ALLOC_FAILED)

/**
 * @brief Check if the string is empty.
 *
 * @param ptr The string.
 * @return bool Flag indicating if the string is empty.
 */
static inline bool str_is_empty(void *ptr)
{
  return str_len(ptr) == 0;
}

bool str_eq(char *const a, char *const b);

// ========== String views ==========

typedef struct
{
  char *start;
  char *end;
} StrView;

#define sv_len(sv) \
  (((sv).start == NULL || (sv).end == NULL) ? 0 : (size_t)((sv).end - (sv).start))

static inline bool sv_is_empty(StrView const sv)
{
  return sv.start == sv.end || sv.start == NULL || sv.end == NULL;
}

StrView sv_from_str(char *str);

StrView sv_trim_left(StrView sv);

StrView sv_trim_right(StrView sv);

static inline StrView sv_trim(StrView sv)
{
  return sv_trim_left(sv_trim_right(sv));
}

bool sv_starts_with(StrView const sv, char const *const prefix);

bool sv_ends_with(StrView const sv, char const *const suffix);

bool sv_strip_prefix(StrView *sv, char const *const prefix);

bool sv_strip_suffix(StrView *sv, char const *const suffix);

bool sv_contains_str(StrView const sv, char const *const needle);

StrView sv_split_at_delim(StrView *sv, char const delim);

#define sv_split_line(sv) sv_split_at_delim((sv), '\n')

char *sv_find(StrView sv, char const c);

static inline bool sv_contains_char(StrView sv, char const c)
{
  return sv_find(sv, c) != NULL;
}

char *sv_rfind(StrView sv, char const c);

StrView sv_slice(StrView sv, size_t const start, size_t const end);

bool sv_eq(StrView const a, StrView const b);

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
} _DeckHeader;

static inline _DeckHeader *_deck_header(void const *ptr)
{
  _DeckHeader *h = (_DeckHeader *)ptr;
  if (h)
    return --h;
  return NULL;
}

void _deck_free_impl(void *ptr);

#define deck_free(ptr) (_deck_free_impl((ptr)), (ptr) = NULL)

uint8_t deck_max_depth(void const *deck);

static inline size_t deck_len(void const *deck)
{
  _DeckHeader *h = _deck_header(deck);
  if (h)
    return h->count;
  return 0;
}

static inline bool deck_is_empty(void *deck)
{
  return deck_len(deck) == 0;
}

void *_deck_push_impl(void *ptr, void *item, size_t const itemsize, uint8_t const depth);

#define deck_push(ptr, item, depth) \
  (((ptr) = _deck_push_impl((ptr), &(item), sizeof(*ptr), (depth))) ? OK : ALLOC_FAILED)

void *_deck_start_new_arr(void *ptr, size_t const itemsize, uint8_t const depth);

#define deck_start_arr(ptr, depth) \
  (((ptr) = _deck_start_new_arr((ptr), sizeof(*(ptr)), (depth))) ? OK : ALLOC_FAILED)

void deck_clear(void const *ptr);

void *_deck_grow_capacity(void *ptr, size_t const itemsize, size_t const n);

#define deck_reserve(ptr, size)                                \
  (((ptr) = _deck_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? OK                                                      \
     : ((size) == 0 ? OK : ALLOC_FAILED))

void deck_flatten(void *ptr);

void deck_graft(void *ptr);

void deck_simplify(void *ptr);

char *_deck_to_str(void        *ptr,
                   size_t const item_size,
                   void (*snprint_item)(void *item, char *dst, size_t len));

#define deck_to_str(ptr, snprint_item) _deck_to_str((ptr), sizeof(*(ptr)), snprint_item)

/* ========== DECK_INIT macro ==========
 *
 * Initialise a deck from a nested parenthesised literal, mirroring the Rust
 * `deck!` macro.  Depths are assigned with the "ruler-sequence" rule: the first
 * group at each nesting level inherits the assigned depth; subsequent siblings
 * compute their own intrinsic depth.
 *
 * Usage:
 *   size_t* d = NULL;
 *   DECK_INIT(d, size_t, (1, 2, 3));                            // depth 1
 *   DECK_INIT(d, size_t, ((1, 2, 3), (4, 5, 6), (7, 8, 9)));   // depth 2
 *   DECK_INIT(d, size_t, (((1, 2), (3, 4)), ((5, 6), (7, 8)))); // depth 3
 *   deck_free(d);
 *
 * Supports nesting up to depth 255 (limited by rescan budget: 3^6 = 729).
 */
/* --- Eval chain: forces 3^6 = 729 rescans --- */
#define _DI_EVAL(...) _DI_E1(_DI_E1(_DI_E1(__VA_ARGS__)))
#define _DI_E1(...) _DI_E2(_DI_E2(_DI_E2(__VA_ARGS__)))
#define _DI_E2(...) _DI_E3(_DI_E3(_DI_E3(__VA_ARGS__)))
#define _DI_E3(...) _DI_E4(_DI_E4(_DI_E4(__VA_ARGS__)))
#define _DI_E4(...) _DI_E5(_DI_E5(_DI_E5(__VA_ARGS__)))
#define _DI_E5(...) __VA_ARGS__

/* --- Core utility --- */
#define _DI_EMPTY()
#define _DI_DEFER(id) id _DI_EMPTY()
#define _DI_EXPAND(...) __VA_ARGS__
#define _DI_CAT(a, b) _DI_CAT_(a, b)
#define _DI_CAT_(a, b) a##b
#define _DI_FIRST(...) _DI_FIRST_(__VA_ARGS__, ~)
#define _DI_FIRST_(a, ...) a
#define _DI_SECOND(...) _DI_SECOND_(__VA_ARGS__, ~, ~)
#define _DI_SECOND_(a, b, ...) b

/* --- Paren detection: _DI_IS_PAREN(x) => 1 if x is (...), else 0 --- */
#define _DI_IS_PAREN_PROBE(...) ~, 1
#define _DI_IS_PAREN(x) _DI_IS_PAREN_CHK(_DI_IS_PAREN_PROBE x)
#define _DI_IS_PAREN_CHK(...) _DI_SECOND(__VA_ARGS__, 0)

/* --- Boolean --- */
#define _DI_NOT(x) _DI_CAT(_DI_NOT_, x)
#define _DI_NOT_0 1
#define _DI_NOT_1 0

/* --- End sentinel (function-like macro; detected via invocation, not ##).
 *     _DI_DONE_CHK is a probe: for empty x it sees () directly; for _DI_END
 *     the sentinel eats () instead.  Either way ~,1 is injected.  For normal
 *     values (including 1.1) neither fires, so _DI_SECOND returns 0. --- */
#define _DI_END(...) ~, 1
#define _DI_DONE_CHK() ~, 1
#define _DI_IS_END(x) _DI_SECOND(_DI_DONE_CHK x(), 0)
#define _DI_CONT(x) _DI_CAT(_DI_CONT_, _DI_IS_PAREN(x))(x)
#define _DI_CONT_1(x) 1
#define _DI_CONT_0(x) _DI_NOT(_DI_IS_END(x))

/* --- Push: main dispatch (CAT-based to avoid IIF prescanning) ---
 *     Depth is threaded as an accumulator: +1 at each paren-strip.
 *     Rest-groups always restart at depth 1 (intrinsic depth). */
#define _DI_PUSH(ptr, type, depth, ...) \
  _DI_CAT(_DI_PC_, _DI_CONT(_DI_FIRST(__VA_ARGS__)))(ptr, type, depth, __VA_ARGS__)
#define _DI_PC_0(ptr, type, depth, ...) \
  (void)((ptr) = _deck_start_new_arr((void *)(ptr), sizeof(type), (uint8_t)(depth)));
#define _DI_PC_1(ptr, type, depth, ...) \
  _DI_CAT(_DI_PP_, _DI_IS_PAREN(_DI_FIRST(__VA_ARGS__)))(ptr, type, depth, __VA_ARGS__)
#define _DI_PP_0(ptr, type, depth, ...) _DI_PUSH_VALS(ptr, type, depth, __VA_ARGS__)
#define _DI_PP_1(ptr, type, depth, first_grp, ...)        \
  _DI_DEFER(_DI_PUSH_I)                                   \
  ()(ptr, type, depth + 1, _DI_EXPAND first_grp, _DI_END) \
    _DI_CAT(_DI_GT_, _DI_CONT(_DI_FIRST(__VA_ARGS__)))(ptr, type, __VA_ARGS__)
#define _DI_PUSH_I() _DI_PUSH

/* --- Push flat values: first gets depth, rest get 0 --- */
#define _DI_PUSH_VALS(ptr, type, depth, first, ...)                            \
  (void)((ptr) = _deck_push_impl(                                              \
           (void *)(ptr), &((type) {first}), sizeof(type), (uint8_t)(depth))); \
  _DI_CAT(_DI_VT_, _DI_CONT(_DI_FIRST(__VA_ARGS__)))(ptr, type, __VA_ARGS__)
#define _DI_PUSH_VALS_REST(ptr, type, first, ...)                                     \
  (void)((ptr) = _deck_push_impl((void *)(ptr), &((type) {first}), sizeof(type), 0)); \
  _DI_CAT(_DI_VT_, _DI_CONT(_DI_FIRST(__VA_ARGS__)))(ptr, type, __VA_ARGS__)
#define _DI_VT_0(ptr, type, ...)
#define _DI_VT_1(ptr, type, ...) _DI_DEFER(_DI_PUSH_VALS_REST_I)()(ptr, type, __VA_ARGS__)
#define _DI_PUSH_VALS_REST_I() _DI_PUSH_VALS_REST

/* --- Rest groups: each restarts at depth 1 --- */
#define _DI_GT_0(ptr, type, ...)
#define _DI_GT_1(ptr, type, ...) _DI_DEFER(_DI_PUSH_REST_GRP_I)()(ptr, type, __VA_ARGS__)
#define _DI_PUSH_REST_GRP(ptr, type, grp, ...) \
  _DI_DEFER(_DI_PUSH_I)                        \
  ()(ptr, type, 1, _DI_EXPAND grp, _DI_END)    \
    _DI_CAT(_DI_GT_, _DI_CONT(_DI_FIRST(__VA_ARGS__)))(ptr, type, __VA_ARGS__)
#define _DI_PUSH_REST_GRP_I() _DI_PUSH_REST_GRP

/* --- Unwrap: strip outer parens if present, pass through bare values --- */
#define _DI_UNWRAP(x) _DI_CAT(_DI_UW_, _DI_IS_PAREN(x))(x)
#define _DI_UW_1(x) _DI_EXPAND x
#define _DI_UW_0(x) x

/* --- Entry point --- */
#define DECK_INIT(ptr, type, data)                                \
  do {                                                            \
    deck_clear((void *)(ptr));                                    \
    _DI_EVAL(_DI_PUSH((ptr), type, 1, _DI_UNWRAP(data), _DI_END)) \
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
} DeckView;

DeckView _dv_from_deck_impl(void *ptr, size_t const item_size, uint8_t const depth);

#define dv_from_deck(ptr, depth) _dv_from_deck_impl((ptr), sizeof(*(ptr)), (depth))

uint8_t dv_depth(DeckView const *const v);

size_t dv_len(DeckView const *const v);

DeckView dv_child(DeckView const *const v);

void const *dv_item_ptr(DeckView const *const v);

bool dv_advance(DeckView *const v);

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
    .start          = deck_len(*deck),
  };
}

#define dw_from_deck(deck, depth) \
  (_dw_from_deck_impl((void **)(&(deck)), (depth), sizeof(*(deck))))

DeckWriter dw_child(DeckWriter *writer);

uint8_t _dw_next_depth(DeckWriter *writer);

Status _dw_push_impl(DeckWriter *writer, void *item);

#define dw_push(writer, item) (_dw_push_impl((writer), &(item)))

void *dw_push_empty(DeckWriter *writer);

static inline void *deck_item_ptr(DeckWriter *writer)
{
  return (char *)(*(writer->deck)) + writer->start * writer->item_size;
}

static inline size_t dw_len(DeckWriter *writer)
{
  return deck_len(*(writer->deck)) - writer->start;
}

Status dw_close(DeckWriter *writer);

Status dw_advance(DeckWriter *writer);

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

DeckView comb_get_input(void *comb, size_t const index);

DeckWriter *comb_get_output(void *comb, size_t const index);

void oh_update(OrcHandle *handle);
