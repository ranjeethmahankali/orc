#pragma once

#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <orc_abi.h>

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

/* malloc guarantees alignment to 2*sizeof(void*) on most platforms (16 on
   64-bit). We use this as the threshold: requests at or below this alignment
   go straight to malloc; larger alignments get the over-allocate-and-align
   treatment. */
#define ORC_SDK_MALLOC_DEFAULT_ALIGN (2u * sizeof(void *))

// ========== Memory ==========

void *orc_sdk_alloc(uint64_t const size, uint64_t const alignment);
void  orc_sdk_free(void *ptr, uint64_t const size, uint64_t const alignment);
void *orc_sdk_realloc(void          *ptr,
                      uint64_t const old_size,
                      uint64_t const new_size,
                      uint64_t const alignment);

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
#define orc_sdk_arr_free(ptr)                                     \
  do {                                                            \
    _OrcSdk_ArrHeader *_h_ = _orc_sdk_arr_header((ptr));          \
    if (_h_)                                                      \
      orc_sdk_free(_h_,                                           \
                   sizeof(*_h_) + _h_->capacity * sizeof(*(ptr)), \
                   ORC_SDK_MALLOC_DEFAULT_ALIGN);                 \
    (ptr) = NULL;                                                 \
  } while (0)

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

OrcError _orc_sdk_arr_remove_impl(void *ptr, size_t const idx, size_t const elemsize);

/**
 * Remove the element at index idx from the array. This preserves the order of the
 * remaining elements.
 */
#define orc_sdk_arr_remove(ptr, idx) _orc_sdk_arr_remove_impl((ptr), (idx), sizeof *(ptr))

/**
 * Push a new value to the end of the array.
 */
#define orc_sdk_arr_push(ptr, val)                                          \
  (((ptr) = _orc_sdk_arr_grow(ptr, sizeof *(ptr)))                          \
     ? ((ptr)[_orc_sdk_arr_header((ptr))->count++] = (val), ORC_ERROR_NONE) \
     : ORC_ERROR_ALLOC_FAILED)

/**
 * Get a pointer pointing past the end of the array.
 */
#define orc_sdk_arr_end(ptr) (ptr) + orc_sdk_arr_len((ptr))

/**
 * Removes the element at the index idx from the array. This is faster than
 * orc_sdk_arr_remove, but doesn't preserve the order of the elements.
 */
#define orc_sdk_arr_swap_remove(ptr, idx)                                            \
  (((ptr) != NULL && (idx) < orc_sdk_arr_len(ptr))                                   \
     ? ((ptr)[(idx)] = (ptr)[--(_orc_sdk_arr_header((ptr))->count)], ORC_ERROR_NONE) \
     : ORC_ERROR_OUT_OF_BOUNDS)

void *_orc_sdk_arr_grow_capacity(void *ptr, size_t const elemsize, size_t const nelems);

/**
 * Reserve memory for the `size` number of elements in the array.
 */
#define orc_sdk_arr_reserve(ptr, size)                                \
  (((ptr) = _orc_sdk_arr_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? ORC_ERROR_NONE                                                 \
     : ((size) == 0 ? ORC_ERROR_NONE : ORC_ERROR_ALLOC_FAILED))

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

OrcError _orc_sdk_arr_remove_range_impl(void        *ptr,
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
 * Remove the last element of the array and assign it to the `dst` pointer. ORC_ERROR_NONE
 * status is returned if the array was not empty. ORC_ERROR_OUT_OF_BOUNDS status is
 * returned if the array was empty.
 */
#define orc_sdk_arr_pop(ptr, dst)                                            \
  (((ptr) != NULL && orc_sdk_arr_len((ptr)) > 0)                             \
     ? (*dst = (ptr)[--(_orc_sdk_arr_header((ptr)))->count], ORC_ERROR_NONE) \
     : ORC_ERROR_OUT_OF_BOUNDS)

/**
 * @brief Check if the array is empty.
 *
 * @param ptr Pointer to the array.
 * @return bool True if the array is empty.
 */
bool orc_sdk_arr_is_empty(void *ptr);

// ========== Hash map ==========

#define ORC_SDK_HMAP_BUCKET_SIZE 8

typedef struct
{
  size_t    hash[ORC_SDK_HMAP_BUCKET_SIZE];
  ptrdiff_t index[ORC_SDK_HMAP_BUCKET_SIZE];
} _OrcSdk_HashBucket;

typedef struct
{
  size_t              n_total;
  size_t              n_used;
  size_t              n_removed;
  size_t              kvsize;
  _OrcSdk_HashBucket *buckets;
  _OrcSdk_HashBucket *temp_buckets;
  size_t             *slots;
  size_t              _padding;  // For aligning the struct to 16 bytes.
} _OrcSdk_HashTableHeader;

_OrcSdk_HashTableHeader *_orc_sdk_hmap_header(void *ptr);

void *_orc_sdk_hmap_grow_size(void *ptr, size_t const kvsize, size_t nelems);

/**
 * Reserve space to store `size` number of entries in the hash table.
 */
#define orc_sdk_hmap_reserve(ptr, size)                            \
  (((ptr) = _orc_sdk_hmap_grow_size((ptr), sizeof *(ptr), (size))) \
     ? ORC_ERROR_NONE                                              \
     : ORC_ERROR_ALLOC_FAILED)

size_t _orc_sdk_hmap_insert_bin_impl(void       **ptr,
                                     size_t const kvsize,
                                     void        *keyptr,
                                     size_t const keysize);

/**
 * Insert the given key value pair into the hash table. The key should be provided as an
 * lvalue to allow for generic type safe functionality. Use this variant to insert keys
 * that are binary value types.
 */
#define orc_sdk_hmap_put(ptr, k, v)                                                  \
  do {                                                                               \
    size_t const idx =                                                               \
      _orc_sdk_hmap_insert_bin_impl((void **)&(ptr), sizeof *(ptr), &(k), sizeof k); \
    (ptr)[idx].value = (v);                                                          \
  } while (0)

/**
 * @brief Get the length of the hash map. This is the number of entries in the hash map.
 *
 * @param ptr Pointer to the hash map.
 * @return size_t
 */
size_t orc_sdk_hmap_len(void *ptr);

void _orc_sdk_hmap_free_impl(void *ptr);

/**
 * Free the hash map, and set the pointer to NULL.
 */
#define orc_sdk_hmap_free(ptr) (_orc_sdk_hmap_free_impl((ptr)), (ptr) = NULL)

void *_orc_sdk_hmap_get_bin_impl(void        *ptr,
                                 size_t const kvsize,
                                 void        *keyptr,
                                 size_t const keysize);

/**
 * Get the pointer to the entry in the hashmap corresponding to the key. If the key
 * doesn't exist in the hash map, NULL is returned. A boid ptr is returned, so it needs to
 * be cast back to the pointer type of the key value pair struct. The key should be passed
 * in as an lvalue. This is required for the API to be generic and typesafe. This only
 * works for keys that are binary value types.
 */
#define orc_sdk_hmap_get(ptr, key) \
  _orc_sdk_hmap_get_bin_impl((ptr), sizeof *(ptr), &(key), sizeof key)

/**
 * Check if the hash map contains the given key.
 */
#define orc_sdk_hmap_contains(ptr, key) \
  (_orc_sdk_hmap_get_bin_impl((ptr), sizeof *(ptr), &(key), sizeof key) != NULL)

bool _orc_sdk_hmap_remove_bin_impl(void        *ptr,
                                   size_t const kvsize,
                                   void        *keyptr,
                                   size_t const keysize);

/**
 * Remove the entry from the hashmap corresponding to the given key. Returns true if the
 * key was present and removed, false if the key was not found.
 */
#define orc_sdk_hmap_remove(ptr, key) \
  _orc_sdk_hmap_remove_bin_impl((ptr), sizeof *(ptr), &(key), sizeof key)

/**
 * @brief Check if the hash map is empty.
 *
 * @param ptr Pointer to the hashmap.
 * @return bool True if the hash map is empty.
 */
static inline bool orc_sdk_hmap_is_empty(void *ptr)
{
  return orc_sdk_hmap_len(ptr) == 0;
}

// ========== Hash set - most functionality just reuses hashmaps. ==========

/**
 * Insert the value into the hash set. Must be a value type.
 */
#define orc_sdk_hset_put(ptr, val)                                                     \
  do {                                                                                 \
    _orc_sdk_hmap_insert_bin_impl((void **)&(ptr), sizeof *(ptr), &(val), sizeof val); \
  } while (0)

/**
 * @brief Get the length of the hash map. This is the number of entries.
 *
 * @param ptr
 * @return size_t
 */
static inline size_t orc_sdk_hset_len(void *ptr)
{
  return orc_sdk_hmap_len(ptr);
}

/**
 * Reserve space for the given number of elements in the hash set.
 */
#define orc_sdk_hset_reserve orc_sdk_hmap_reserve

/**
 * Check if the hash set contains the given value.
 */
#define orc_sdk_hset_contains orc_sdk_hmap_contains

/**
 * Remove the given value from the hash set. If the value is not present in the hash set,
 * it will be unaffected.
 */
#define orc_sdk_hset_remove orc_sdk_hmap_remove

/**
 * Free hte hash set, and set the pointer to NULL.
 */
#define orc_sdk_hset_free orc_sdk_hmap_free

/**
 * @brief Check if the hash set is empty.
 *
 * @param ptr Pointer to the hash set.
 * @return bool True if the hash set is empty.
 */
static inline bool orc_sdk_hset_is_empty(void *ptr)
{
  return orc_sdk_hset_len(ptr) == 0;
}

// ========== String ==========

#define orc_str_free(ptr) orc_sdk_arr_free((ptr))

static inline size_t orc_str_len(void *ptr)
{
  _OrcSdk_ArrHeader *h = _orc_sdk_arr_header(ptr);
  if (h)
    return h->count == 0 ? 0 : h->count - 1;
  return 0;
}

OrcError _orc_str_remove_impl(char *const ptr, size_t const idx);

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
  (((ptr) = _orc_str_push_impl((ptr), (ch))) ? ORC_ERROR_NONE : ORC_ERROR_ALLOC_FAILED)

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
#define orc_str_push_str(dst, tail)                                 \
  (((dst) = _orc_str_push_str_impl((dst), (tail))) ? ORC_ERROR_NONE \
                                                   : ORC_ERROR_ALLOC_FAILED)

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

OrcError orc_sdk_sv_read_bytes(OrcStrView *sv, void *dst, size_t const count);

// ========== Queue ==========

typedef struct
{
  size_t back;
  size_t capacity;
  size_t front;
  size_t _padding;
} _OrcSdk_QueueHeader;

static inline _OrcSdk_QueueHeader *_orc_sdk_que_header(void *ptr)
{
  _OrcSdk_QueueHeader *h = (_OrcSdk_QueueHeader *)ptr;
  if (h)
    return --h;
  return NULL;
}

void *_orc_sdk_que_push_grow(void *ptr, size_t const elemsize);

/**
 * @brief Check if the queue is empty.
 *
 * @param ptr Pointer to the queue.
 * @return bool True if the queue is empty.
 */
bool orc_sdk_que_is_empty(void *ptr);

size_t _orc_sdk_que_pushed_back(void *ptr);

/**
 * Push the new value to the back of the queue. Status is returned indicating whether the
 * operation was successful.
 */
#define orc_sdk_que_push(ptr, val)                                      \
  (((ptr) = _orc_sdk_que_push_grow(ptr, sizeof *(ptr)))                 \
     ? ((ptr)[_orc_sdk_que_pushed_back((ptr))] = (val), ORC_ERROR_NONE) \
     : ORC_ERROR_ALLOC_FAILED)

size_t _orc_sdk_que_popped_front(void *ptr);

/**
 * Remove an element from the front of the queue and
 */
#define orc_sdk_que_pop(ptr, dst) \
  (orc_sdk_que_is_empty((ptr))    \
     ? ORC_ERROR_OUT_OF_BOUNDS    \
     : (*(dst) = (ptr)[_orc_sdk_que_popped_front((ptr))], ORC_ERROR_NONE))

/**
 * @brief Get the length of the queue. This is the remaining items in the queue.
 *
 * @param ptr The pointer to the queue.
 * @return size_t The length.
 */
size_t orc_sdk_que_len(void *ptr);

/**
 * Free the queue, and set the pointer to NULL.
 */
#define orc_sdk_que_free(ptr) (free(_orc_sdk_que_header((ptr))), (ptr) = NULL)

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

void *_orc_sdk_deck_push_impl(void         *ptr,
                              void         *item,
                              size_t const  itemsize,
                              uint8_t const depth);

#define orc_sdk_deck_push(ptr, item, depth)                                 \
  (((ptr) = _orc_sdk_deck_push_impl((ptr), &(item), sizeof(*ptr), (depth))) \
     ? ORC_ERROR_NONE                                                       \
     : ORC_ERROR_ALLOC_FAILED)

void *_orc_sdk_deck_start_new_arr(void *ptr, size_t const itemsize, uint8_t const depth);

#define orc_sdk_deck_start_arr(ptr, depth)                               \
  (((ptr) = _orc_sdk_deck_start_new_arr((ptr), sizeof(*(ptr)), (depth))) \
     ? ORC_ERROR_NONE                                                    \
     : ORC_ERROR_ALLOC_FAILED)

void orc_sdk_deck_clear(void const *ptr);

void *_orc_sdk_deck_grow_capacity(void *ptr, size_t const itemsize, size_t const n);

#define orc_sdk_deck_reserve(ptr, size)                                \
  (((ptr) = _orc_sdk_deck_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? ORC_ERROR_NONE                                                  \
     : ((size) == 0 ? ORC_ERROR_NONE : ORC_ERROR_ALLOC_FAILED))

void orc_sdk_deck_flatten(void *ptr);

OrcError orc_sdk_deck_graft(void *ptr);

void orc_sdk_deck_simplify(void *ptr);

void orc_sdk_deck_calc_strides(_OrcSdk_DeckHeader *h);

char *_orc_sdk_deck_to_str(void const  *ptr,
                           size_t const item_size,
                           void (*snprint_item)(void *item, char *dst, size_t len));

#define orc_sdk_deck_to_str(ptr, snprint_item) \
  _orc_sdk_deck_to_str((ptr), sizeof(*(ptr)), snprint_item)

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
  (void)((ptr) =                                \
           _orc_sdk_deck_start_new_arr((void *)(ptr), sizeof(type), (uint8_t)(depth)));
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
  (void)((ptr) = _orc_sdk_deck_push_impl(                                            \
           (void *)(ptr), &((type) {first}), sizeof(type), (uint8_t)(depth)));       \
  _ORC_SDK_DI_CAT(_ORC_SDK_DI_VT_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__))) \
  (ptr, type, __VA_ARGS__)
#define _ORC_SDK_DI_PUSH_VALS_REST(ptr, type, first, ...)                               \
  (void)((ptr) =                                                                        \
           _orc_sdk_deck_push_impl((void *)(ptr), &((type) {first}), sizeof(type), 0)); \
  _ORC_SDK_DI_CAT(_ORC_SDK_DI_VT_, _ORC_SDK_DI_CONT(_ORC_SDK_DI_FIRST(__VA_ARGS__)))    \
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
    orc_sdk_deck_clear((void *)(ptr));                                             \
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

OrcSdk_DeckView _orc_sdk_dv_from_deck_impl(void         *ptr,
                                           size_t const  item_size,
                                           uint8_t const depth);

#define orc_sdk_dv_from_deck(ptr, depth) \
  _orc_sdk_dv_from_deck_impl((ptr), sizeof(*(ptr)), (depth))

uint8_t orc_sdk_dv_depth(OrcSdk_DeckView const *const v);

size_t orc_sdk_dv_len(OrcSdk_DeckView const *const v);

OrcSdk_DeckView orc_sdk_dv_child(OrcSdk_DeckView const *const v);

void const *orc_sdk_dv_item_ptr(OrcSdk_DeckView const *const v);

bool orc_sdk_dv_advance(OrcSdk_DeckView *const v);

// ========== DeckWriter ==========

typedef struct
{
  void  **deck;
  size_t  item_size;
  uint8_t depth;
  bool    has_next_depth;
  uint8_t next_depth;
  size_t  start;
} OrcSdk_DeckWriter;

static inline OrcSdk_DeckWriter _orc_sdk_dw_from_deck_impl(void        **deck,
                                                           uint8_t const depth,
                                                           size_t const  item_size)
{
  return (OrcSdk_DeckWriter) {
    .deck           = deck,
    .item_size      = item_size,
    .depth          = depth,
    .has_next_depth = true,
    .next_depth     = depth,
    .start          = orc_sdk_deck_len(*deck),
  };
}

#define orc_sdk_dw_from_deck(deck, depth) \
  (_orc_sdk_dw_from_deck_impl((void **)(&(deck)), (depth), sizeof(*(deck))))

OrcSdk_DeckWriter orc_sdk_dw_child(OrcSdk_DeckWriter *writer);

uint8_t _orc_sdk_dw_next_depth(OrcSdk_DeckWriter *writer);

OrcError _orc_sdk_dw_push_impl(OrcSdk_DeckWriter *writer, void *item);

#define orc_sdk_dw_push(writer, item) (_orc_sdk_dw_push_impl((writer), &(item)))

void *orc_sdk_dw_push_empty(OrcSdk_DeckWriter *writer);

static inline void *orc_sdk_deck_item_ptr(OrcSdk_DeckWriter *writer)
{
  return (char *)(*(writer->deck)) + writer->start * writer->item_size;
}

static inline size_t orc_sdk_dw_len(OrcSdk_DeckWriter *writer)
{
  return orc_sdk_deck_len(*(writer->deck)) - writer->start;
}

OrcError orc_sdk_dw_close(OrcSdk_DeckWriter *writer);

OrcError orc_sdk_dw_advance(OrcSdk_DeckWriter *writer);

// ========== Dims (Units) ==========

bool orc_sdk_dims_equal(OrcDims const a, OrcDims const b);

void orc_sdk_dims_multiply(OrcDims const a, OrcDims const b, OrcDims out);

void orc_sdk_dims_divide(OrcDims const a, OrcDims const b, OrcDims out);

void orc_sdk_dims_pow(OrcDims const a, int const pow, OrcDims out);

// ========== Combinations ==========

void *orc_sdk_comb_init(OrcHandle const **inputs,
                        uint8_t const    *input_depths,
                        size_t const      n_inputs,
                        OrcHandle       **outputs,
                        uint8_t const    *output_depths,
                        size_t const      n_outputs);

void orc_sdk_comb_free(void *comb);

void *orc_sdk_comb_advance(void *comb);

OrcSdk_DeckView orc_sdk_comb_get_input(void *comb, size_t const index);

OrcSdk_DeckWriter *orc_sdk_comb_get_output(void *comb, size_t const index);

// ========== Other helpers ==========

void orc_sdk_oh_update(OrcHandle *handle);

void orc_sdk_report_message(uint64_t const        ctx,
                            OrcMessageLevel const level,
                            char const           *msg);

/**
 * Meant to be called by a plugin when it wants to create a proxy deck of a type that it
 * doesn't know. This function requests the host to defer the task to the plugin that owns
 * the type.
 */
OrcError orc_sdk_host_create_proxy_deck(OrcHandle const   *inputs,
                                        uint64_t const     n_inputs,
                                        OrcProxyType const proxy_type,
                                        OrcHandle const   *proxy,
                                        OrcHandle         *out);

OrcError orc_sdk_host_serial_write(uint64_t const ctx,
                                   void const    *buf,
                                   uint64_t const buf_len);

// ========== ABI helpers ==========

typedef void (*ItemFreeFn)(void *);
typedef void (*CopyItemsFn)(void const *src, void *dst, size_t const n_items);

typedef struct
{
  uint64_t    item_size;  // Must be non zero.
  CopyItemsFn copy_fn;    // Must be non-NULL.
  ItemFreeFn  free_fn;    // NULL for value types that don't own any resources.
} OrcSdkTypeInfo;

typedef OrcSdkTypeInfo (*OrcSdkTypeCallbacksGetterFn)(OrcTypeId const id);

void orc_sdk_init(OrcHost const *host, OrcSdkTypeCallbacksGetterFn type_fn);

OrcError orc_sdk_handle_alloc(OrcTypeId const type_id, OrcHandle *const out);

OrcError orc_sdk_handle_free(OrcHandle *const handle);

OrcError orc_sdk_deck_from_proxy(OrcHandle const   *inputs,
                                 uint64_t const     n_inputs,
                                 OrcProxyType const proxy_type,
                                 OrcHandle const   *proxy,
                                 OrcHandle         *out);

// ==================== Serialization Helpers ====================

OrcError orc_sdk_serialize_handle_header(uint64_t const ctx, OrcHandle const *handle);

OrcError orc_sdk_deserialize_handle_header(uint64_t const ctx,
                                           OrcStrView    *src,
                                           OrcHandle     *out,
                                           OrcMark      **out_marks);
